package org.fcast.android.sender.runtime

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertNotNull
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [34], application = android.app.Application::class)
class StatusParserTest {

    @Test
    fun running_yields_runningState() {
        val s = StatusParser.parse("""{"state":"running"}""")
        assertEquals("running", s.state)
        assertNull(s.message)
        assertNull(s.extra)
    }

    @Test
    fun error_withMessage_isPreserved() {
        val s = StatusParser.parse("""{"state":"error","message":"boom"}""")
        assertEquals("error", s.state)
        assertEquals("boom", s.message)
    }

    @Test
    fun extra_objectIsPreservedAsRawJson() {
        val raw = """{"state":"running","extra":{"x":1}}"""
        val s = StatusParser.parse(raw)
        assertNotNull(s.extra)
        assertEquals(1, s.extra?.optInt("x"))
    }

    @Test
    fun unparseable_isMappedToError() {
        val s = StatusParser.parse("not json")
        assertEquals("error", s.state)
        assertNotNull(s.message) // contains a JSONException message
    }

    @Test
    fun emptyMessage_isCoercedToNull() {
        val s = StatusParser.parse("""{"state":"running","message":""}""")
        assertNull(s.message)
    }
}
