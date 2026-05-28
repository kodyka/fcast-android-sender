package org.fcast.android.sender.capture

import android.graphics.SurfaceTexture
import android.hardware.display.DisplayManager
import android.hardware.display.VirtualDisplay
import android.media.projection.MediaProjection
import android.opengl.EGL14
import android.opengl.EGLConfig
import android.opengl.EGLContext
import android.opengl.EGLDisplay
import android.opengl.EGLSurface
import android.opengl.GLES11Ext.GL_TEXTURE_EXTERNAL_OES
import android.opengl.GLES20.*
import android.os.Handler
import android.os.HandlerThread
import android.util.Log
import android.view.Surface
import androidx.annotation.WorkerThread
import org.fcast.android.sender.MainActivity
import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.nio.FloatBuffer
import java.util.concurrent.atomic.AtomicBoolean

/**
 * Owns the EGL/GL pipeline that converts the projected display into YUV planes
 * and ships them to native code.
 *
 * Instantiated by the coordinator (one per capture session); shutdown by the
 * coordinator when the user stops, the projection callback fires, or the
 * activity is destroyed.
 *
 * **Threading:** all GL work happens on the [glThread] HandlerThread. Public
 * `start` / `shutdown` are safe from the main thread.
 */
class CaptureEngine {

    @Volatile private var running = false

    private val glThread = HandlerThread("CaptureEngineGL").also { it.start() }
    private val glHandler = Handler(glThread.looper)

    private var virtualDisplay: VirtualDisplay? = null
    private var surfaceTexture: SurfaceTexture? = null
    private var surface: Surface? = null

    private var eglDisplay: EGLDisplay = EGL14.EGL_NO_DISPLAY
    private var eglContext: EGLContext = EGL14.EGL_NO_CONTEXT
    private var eglSurface: EGLSurface = EGL14.EGL_NO_SURFACE

    private var yFramebuffer: Framebuffer? = null
    private var uFramebuffer: Framebuffer? = null
    private var vFramebuffer: Framebuffer? = null

    private var yProg: Program? = null
    private var uProg: Program? = null
    private var vProg: Program? = null

    private var vboId = 0
    private var oesTexId = 0

    private val quad = floatArrayOf(
        -1f, -1f, 0f, 1f,
         1f, -1f, 1f, 1f,
        -1f,  1f, 0f, 0f,
         1f,  1f, 1f, 0f
    )

    private var lastFrameNanos: Long = 0L
    private var minIntervalNanos: Long = 0L
    private val shouldCapture = AtomicBoolean(false)

    class Dimensions(val width: Int, val height: Int) {
        fun scale(maxDims: Dimensions): Dimensions {
            if (width <= maxDims.width && height <= maxDims.height) {
                return this
            }
            val ratio = width.toFloat() / height.toFloat()
            val maxRatio = maxDims.width.toFloat() / maxDims.height.toFloat()
            return if (ratio > maxRatio) {
                Dimensions(maxDims.width, (maxDims.width / ratio).toInt())
            } else {
                Dimensions((maxDims.height * ratio).toInt(), maxDims.height)
            }
        }
    }

    class Program(frag: String, isChroma: Boolean) {
        var program: Int = 0
        var position: Int = 0
        var texCoord: Int = 0
        var texMatrix: Int = 0
        var textureUniform: Int = 0
        var srcSize: Int = 0

        init {
            program = createProgram(frag)
            position = glGetAttribLocation(program, "aPosition")
            texCoord = glGetAttribLocation(program, "aTexCoord")
            texMatrix = glGetUniformLocation(program, "uTexMatrix")
            textureUniform = glGetUniformLocation(program, "sTexture")
            if (isChroma) {
                srcSize = glGetUniformLocation(program, "srcSize")
            }
        }

        private fun loadShader(shaderType: Int, source: String): Int {
            var shader = glCreateShader(shaderType)
            if (shader != 0) {
                glShaderSource(shader, source)
                glCompileShader(shader)
                val compiled = IntArray(1)
                glGetShaderiv(shader, GL_COMPILE_STATUS, compiled, 0)
                if (compiled[0] == 0) {
                    Log.e(TAG, "Could not compile shader $shaderType: " + glGetShaderInfoLog(shader))
                    glDeleteShader(shader)
                    shader = 0
                }
            }
            return shader
        }

        private fun createProgram(fragmentSource: String): Int {
            val vert = loadShader(GL_VERTEX_SHADER, vertexShader)
            if (vert == 0) return 0
            val frag = loadShader(GL_FRAGMENT_SHADER, fragmentSource)
            if (frag == 0) {
                glDeleteShader(vert)
                return 0
            }
            var program = glCreateProgram()
            if (program != 0) {
                glAttachShader(program, vert)
                glAttachShader(program, frag)
                glLinkProgram(program)
                val linkStatus = IntArray(1)
                glGetProgramiv(program, GL_LINK_STATUS, linkStatus, 0)
                if (linkStatus[0] != GL_TRUE) {
                    Log.e(TAG, "Could not link program: " + glGetProgramInfoLog(program))
                    glDeleteProgram(program)
                    program = 0
                }
            }
            glDeleteShader(vert)
            glDeleteShader(frag)
            return program
        }
    }

    class Framebuffer(val dims: Dimensions) {
        var fboId: Int = 0
        var texId: Int = 0
        var buf: ByteBuffer = ByteBuffer.allocateDirect(1)

        init {
            val fbos = IntArray(1)
            val texs = IntArray(1)
            glGenFramebuffers(1, fbos, 0)
            glGenTextures(1, texs, 0)
            fboId = fbos[0]
            texId = texs[0]

            glBindTexture(GL_TEXTURE_2D, texId)
            glTexImage2D(GL_TEXTURE_2D, 0, GL_R8, dims.width, dims.height, 0, GL_RED, GL_UNSIGNED_BYTE, null)
            glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_LINEAR)
            glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_LINEAR)
            glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_S, GL_CLAMP_TO_EDGE)
            glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_T, GL_CLAMP_TO_EDGE)

            glBindFramebuffer(GL_FRAMEBUFFER, fboId)
            glFramebufferTexture2D(GL_FRAMEBUFFER, GL_COLOR_ATTACHMENT0, GL_TEXTURE_2D, texId, 0)

            val status = glCheckFramebufferStatus(GL_FRAMEBUFFER)
            if (status != GL_FRAMEBUFFER_COMPLETE) {
                throw RuntimeException("FBO setup failed: $status")
            }
            glBindFramebuffer(GL_FRAMEBUFFER, 0)
        }

        fun readPixels() {
            glBindFramebuffer(GL_FRAMEBUFFER, fboId)
            if (buf.capacity() < dims.width * dims.height) {
                buf = ByteBuffer.allocateDirect(dims.width * dims.height)
            }
            buf.position(0)
            glReadPixels(0, 0, dims.width, dims.height, GL_RED, GL_UNSIGNED_BYTE, buf)
            glBindFramebuffer(GL_FRAMEBUFFER, 0)
        }
    }

    fun start(
        projection: MediaProjection,
        config: CaptureConfig,
        onStarted: (width: Int, height: Int) -> Unit,
        onFatalError: (reason: String) -> Unit,
    ) {
        check(!running) { "CaptureEngine.start called twice" }
        running = true
        minIntervalNanos = config.minIntervalNanos

        glHandler.post {
            try {
                val metrics = android.content.res.Resources.getSystem().displayMetrics
                val srcWidth = metrics.widthPixels
                val srcHeight = metrics.heightPixels
                val srcDensity = metrics.densityDpi

                val maxDims = Dimensions(config.scaleWidth, config.scaleHeight)
                val srcDims = Dimensions(srcWidth, srcHeight)
                val downscaledDims = srcDims.scale(maxDims)
                val uvDims = Dimensions(downscaledDims.width / 2, downscaledDims.height / 2)

                initEgl(downscaledDims)

                oesTexId = createOesTexture()

                yFramebuffer = Framebuffer(downscaledDims)
                uFramebuffer = Framebuffer(uvDims)
                vFramebuffer = Framebuffer(uvDims)

                yProg = Program(fragmentShaderY, false)
                uProg = Program(fragmentShaderU, true)
                vProg = Program(fragmentShaderV, true)

                val vbos = IntArray(1)
                glGenBuffers(1, vbos, 0)
                vboId = vbos[0]

                glBindBuffer(GL_ARRAY_BUFFER, vboId)
                val vertexBuffer = ByteBuffer.allocateDirect(quad.size * 4).order(ByteOrder.nativeOrder()).asFloatBuffer()
                vertexBuffer.put(quad)
                vertexBuffer.position(0)
                glBufferData(GL_ARRAY_BUFFER, quad.size * 4, vertexBuffer, GL_STATIC_DRAW)
                glBindBuffer(GL_ARRAY_BUFFER, 0)

                val st = SurfaceTexture(oesTexId).also { surfaceTexture = it }
                st.setDefaultBufferSize(srcDims.width, srcDims.height)
                st.setOnFrameAvailableListener({
                    onFrameAvailable()
                }, glHandler)

                val surf = Surface(st).also { surface = it }

                virtualDisplay = projection.createVirtualDisplay(
                    "ScreenCapture",
                    srcDims.width,
                    srcDims.height,
                    srcDensity,
                    DisplayManager.VIRTUAL_DISPLAY_FLAG_AUTO_MIRROR or
                            DisplayManager.VIRTUAL_DISPLAY_FLAG_PUBLIC or
                            DisplayManager.VIRTUAL_DISPLAY_FLAG_PRESENTATION,
                    surf,
                    null,
                    null
                )

                EGL14.eglMakeCurrent(eglDisplay, EGL14.EGL_NO_SURFACE, EGL14.EGL_NO_SURFACE, EGL14.EGL_NO_CONTEXT)

                shouldCapture.set(true)
                onStarted(downscaledDims.width, downscaledDims.height)
            } catch (t: Throwable) {
                Log.e(TAG, "GL init failed", t)
                running = false
                onFatalError(t.message ?: "GL init failed")
            }
        }
    }

    fun shutdown() {
        if (!running) return
        running = false
        shouldCapture.set(false)
        glHandler.post {
            try {
                virtualDisplay?.release()
                virtualDisplay = null

                surface?.release()
                surface = null

                surfaceTexture?.release()
                surfaceTexture = null

                releaseGl()
            } catch (t: Throwable) {
                Log.w(TAG, "shutdown raced with a frame", t)
            }
        }
        glThread.quitSafely()
        try { glThread.join(1000L) } catch (_: InterruptedException) {}
    }

    @WorkerThread
    private fun initEgl(dims: Dimensions) {
        eglDisplay = EGL14.eglGetDisplay(EGL14.EGL_DEFAULT_DISPLAY)
        require(eglDisplay != EGL14.EGL_NO_DISPLAY) { "eglGetDisplay failed" }

        val version = IntArray(2)
        EGL14.eglInitialize(eglDisplay, version, 0, version, 1)

        val configs = arrayOfNulls<EGLConfig>(1)
        val numConfig = IntArray(1)
        EGL14.eglChooseConfig(
            eglDisplay,
            EGL_CONFIG_ATTRIBS, 0, configs, 0, 1, numConfig, 0
        )

        val ctxAttribs = intArrayOf(EGL14.EGL_CONTEXT_CLIENT_VERSION, 3, EGL14.EGL_NONE)
        eglContext = EGL14.eglCreateContext(
            eglDisplay, configs[0], EGL14.EGL_NO_CONTEXT, ctxAttribs, 0
        )

        val pbufAttribs = intArrayOf(EGL14.EGL_WIDTH, dims.width, EGL14.EGL_HEIGHT, dims.height, EGL14.EGL_NONE)
        eglSurface = EGL14.eglCreatePbufferSurface(eglDisplay, configs[0], pbufAttribs, 0)
        require(EGL14.eglMakeCurrent(eglDisplay, eglSurface, eglSurface, eglContext)) {
            "eglMakeCurrent failed: " + EGL14.eglGetError()
        }
    }

    private fun createOesTexture(): Int {
        val tex = IntArray(1)
        glGenTextures(1, tex, 0)
        glBindTexture(GL_TEXTURE_EXTERNAL_OES, tex[0])
        glTexParameteri(GL_TEXTURE_EXTERNAL_OES, GL_TEXTURE_MIN_FILTER, GL_LINEAR)
        glTexParameteri(GL_TEXTURE_EXTERNAL_OES, GL_TEXTURE_MAG_FILTER, GL_LINEAR)
        glTexParameteri(GL_TEXTURE_EXTERNAL_OES, GL_TEXTURE_WRAP_S, GL_CLAMP_TO_EDGE)
        glTexParameteri(GL_TEXTURE_EXTERNAL_OES, GL_TEXTURE_WRAP_T, GL_CLAMP_TO_EDGE)
        glBindTexture(GL_TEXTURE_EXTERNAL_OES, 0)
        return tex[0]
    }

    @WorkerThread
    private fun onFrameAvailable() {
        if (!running || !shouldCapture.get()) return
        val now = System.nanoTime()
        if (now - lastFrameNanos < minIntervalNanos) return
        lastFrameNanos = now

        try {
            pumpOneFrame()
        } catch (e: RuntimeException) {
            Log.e(TAG, "pumpOneFrame failed: $e")
        }
    }

    @WorkerThread
    private fun pumpOneFrame() {
        val st = surfaceTexture ?: return
        val yFb = yFramebuffer ?: return
        val uFb = uFramebuffer ?: return
        val vFb = vFramebuffer ?: return
        val yP = yProg ?: return
        val uP = uProg ?: return
        val vP = vProg ?: return

        if (!EGL14.eglMakeCurrent(eglDisplay, eglSurface, eglSurface, eglContext)) {
            throw RuntimeException("EGL make current failed: " + EGL14.eglGetError())
        }

        st.updateTexImage()

        val texMatrix = FloatArray(16)
        st.getTransformMatrix(texMatrix)

        renderToFbWithProg(oesTexId, yFb, yP, texMatrix)
        renderToFbWithProg(oesTexId, uFb, uP, texMatrix)
        renderToFbWithProg(oesTexId, vFb, vP, texMatrix)

        yFb.readPixels()
        uFb.readPixels()
        vFb.readPixels()

        EGL14.eglMakeCurrent(eglDisplay, EGL14.EGL_NO_SURFACE, EGL14.EGL_NO_SURFACE, EGL14.EGL_NO_CONTEXT)

        MainActivity.nativeProcessFrame(
            yFb.dims.width,
            yFb.dims.height,
            yFb.buf,
            uFb.buf,
            vFb.buf
        )
    }

    private fun renderToFbWithProg(oesTexId: Int, fb: Framebuffer, prog: Program, texMatrix: FloatArray) {
        glBindFramebuffer(GL_FRAMEBUFFER, fb.fboId)
        glViewport(0, 0, fb.dims.width, fb.dims.height)

        glUseProgram(prog.program)

        glBindBuffer(GL_ARRAY_BUFFER, vboId)

        glEnableVertexAttribArray(prog.position)
        glVertexAttribPointer(prog.position, 2, GL_FLOAT, false, 16, 0)

        glEnableVertexAttribArray(prog.texCoord)
        glVertexAttribPointer(prog.texCoord, 2, GL_FLOAT, false, 16, 8)

        glUniformMatrix4fv(prog.texMatrix, 1, false, texMatrix, 0)

        if (prog.srcSize != 0) {
            val metrics = android.content.res.Resources.getSystem().displayMetrics
            glUniform2f(prog.srcSize, metrics.widthPixels.toFloat(), metrics.heightPixels.toFloat())
        }

        glActiveTexture(GL_TEXTURE0)
        glBindTexture(GL_TEXTURE_EXTERNAL_OES, oesTexId)
        glUniform1i(prog.textureUniform, 0)

        glDrawArrays(GL_TRIANGLE_STRIP, 0, 4)

        glDisableVertexAttribArray(prog.position)
        glDisableVertexAttribArray(prog.texCoord)
        glBindBuffer(GL_ARRAY_BUFFER, 0)

        glBindTexture(GL_TEXTURE_EXTERNAL_OES, 0)

        glBindFramebuffer(GL_FRAMEBUFFER, 0)
    }

    @WorkerThread
    private fun releaseGl() {
        if (yProg != null) { glDeleteProgram(yProg!!.program); yProg = null }
        if (uProg != null) { glDeleteProgram(uProg!!.program); uProg = null }
        if (vProg != null) { glDeleteProgram(vProg!!.program); vProg = null }

        val fbos = intArrayOf(
            yFramebuffer?.fboId ?: 0,
            uFramebuffer?.fboId ?: 0,
            vFramebuffer?.fboId ?: 0
        )
        if (fbos[0] != 0 || fbos[1] != 0 || fbos[2] != 0) {
            glDeleteFramebuffers(3, fbos, 0)
        }
        yFramebuffer = null
        uFramebuffer = null
        vFramebuffer = null

        val texs = intArrayOf(
            oesTexId,
            yFramebuffer?.texId ?: 0,
            uFramebuffer?.texId ?: 0,
            vFramebuffer?.texId ?: 0
        )
        glDeleteTextures(4, texs, 0)
        oesTexId = 0

        if (eglSurface != EGL14.EGL_NO_SURFACE) { EGL14.eglDestroySurface(eglDisplay, eglSurface); eglSurface = EGL14.EGL_NO_SURFACE }
        if (eglContext != EGL14.EGL_NO_CONTEXT) { EGL14.eglDestroyContext(eglDisplay, eglContext); eglContext = EGL14.EGL_NO_CONTEXT }
        if (eglDisplay != EGL14.EGL_NO_DISPLAY) { EGL14.eglTerminate(eglDisplay); eglDisplay = EGL14.EGL_NO_DISPLAY }
    }

    companion object {
        private const val TAG = "CaptureEngine"

        private val EGL_CONFIG_ATTRIBS = intArrayOf(
            EGL14.EGL_RENDERABLE_TYPE, EGL14.EGL_OPENGL_ES3_BIT_KHR,
            EGL14.EGL_RED_SIZE,        8,
            EGL14.EGL_GREEN_SIZE,      8,
            EGL14.EGL_BLUE_SIZE,       8,
            EGL14.EGL_ALPHA_SIZE,      8,
            EGL14.EGL_NONE
        )

        private const val vertexShader = """#extension GL_OES_EGL_image_external : require
attribute vec4 aPosition;
attribute vec4 aTexCoord;
uniform mat4 uTexMatrix;
varying vec2 vTexCoord;
void main() {
    gl_Position = aPosition;
    vTexCoord = (uTexMatrix * aTexCoord).xy;
}"""

        private const val fragShaderHeader = """#extension GL_OES_EGL_image_external : require
precision mediump float;
varying vec2 vTexCoord;
uniform samplerExternalOES sTexture;"""

        private const val fragmentShaderY = fragShaderHeader + """
void main() {
    vec3 rgb = texture2D(sTexture, vTexCoord).rgb;
    float y = 0.2126 * rgb.r + 0.7152 * rgb.g + 0.0722 * rgb.b;
    gl_FragColor = vec4(y, 0.0, 0.0, 0.0);
}"""

        private const val subsampledRgb = """
    vec2 step = 1.0 / srcSize;
    vec3 rgbQ1 = texture2D(sTexture, vTexCoord).rgb;
    vec3 rgbQ2 = texture2D(sTexture, vTexCoord + vec2(step.x, 0.0)).rgb;
    vec3 rgbQ3 = texture2D(sTexture, vTexCoord + vec2(0.0, step.y)).rgb;
    vec3 rgbQ4 = texture2D(sTexture, vTexCoord + vec2(step.x, step.y)).rgb;
    vec3 rgb = (rgbQ1 + rgbQ2 + rgbQ3 + rgbQ4) * 0.25;"""

        private const val fragmentShaderU = fragShaderHeader + """
uniform vec2 srcSize;
void main() {""" + subsampledRgb + """
    float u = -0.1146 * rgb.r - 0.3854 * rgb.g + 0.5 * rgb.b + 0.5;
    gl_FragColor = vec4(u, 0.0, 0.0, 0.0);
}"""

        private const val fragmentShaderV = fragShaderHeader + """
uniform vec2 srcSize;
void main() {""" + subsampledRgb + """
    float v = 0.5 * rgb.r - 0.4542 * rgb.g - 0.0458 * rgb.b + 0.5;
    gl_FragColor = vec4(v, 0.0, 0.0, 0.0);
}"""
    }
}
