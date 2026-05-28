#include <gst/gst.h>
#include <gio/gio.h>

#define GST_G_IO_MODULE_DECLARE(name) \
extern void G_PASTE(g_io_, G_PASTE(name, _load)) (gpointer data)

#define GST_G_IO_MODULE_LOAD(name) \
G_PASTE(g_io_, G_PASTE(name, _load)) (NULL)

/* Declaration of static plugins */
  GST_PLUGIN_STATIC_DECLARE(coreelements);  GST_PLUGIN_STATIC_DECLARE(app);  GST_PLUGIN_STATIC_DECLARE(audioconvert);  GST_PLUGIN_STATIC_DECLARE(audiomixer);  GST_PLUGIN_STATIC_DECLARE(audiorate);  GST_PLUGIN_STATIC_DECLARE(audioresample);  GST_PLUGIN_STATIC_DECLARE(audiotestsrc);  GST_PLUGIN_STATIC_DECLARE(compositor);  GST_PLUGIN_STATIC_DECLARE(gio);  GST_PLUGIN_STATIC_DECLARE(rawparse);  GST_PLUGIN_STATIC_DECLARE(typefindfunctions);  GST_PLUGIN_STATIC_DECLARE(videoconvertscale);  GST_PLUGIN_STATIC_DECLARE(videorate);  GST_PLUGIN_STATIC_DECLARE(videotestsrc);  GST_PLUGIN_STATIC_DECLARE(volume);  GST_PLUGIN_STATIC_DECLARE(videofilter);  GST_PLUGIN_STATIC_DECLARE(deinterlace);  GST_PLUGIN_STATIC_DECLARE(videobox);  GST_PLUGIN_STATIC_DECLARE(videocrop);  GST_PLUGIN_STATIC_DECLARE(videomixer);  GST_PLUGIN_STATIC_DECLARE(playback);  GST_PLUGIN_STATIC_DECLARE(uriplaylistbin);  GST_PLUGIN_STATIC_DECLARE(tcp);  GST_PLUGIN_STATIC_DECLARE(rtsp);  GST_PLUGIN_STATIC_DECLARE(rtp);  GST_PLUGIN_STATIC_DECLARE(rtpmanager);  GST_PLUGIN_STATIC_DECLARE(udp);  GST_PLUGIN_STATIC_DECLARE(dtls);  GST_PLUGIN_STATIC_DECLARE(srtp);  GST_PLUGIN_STATIC_DECLARE(srt);  GST_PLUGIN_STATIC_DECLARE(webrtc);  GST_PLUGIN_STATIC_DECLARE(nice);  GST_PLUGIN_STATIC_DECLARE(rsrtp);  GST_PLUGIN_STATIC_DECLARE(rsrtsp);  GST_PLUGIN_STATIC_DECLARE(rswebrtc);

/* Declaration of static gio modules */
  GST_G_IO_MODULE_DECLARE(openssl);

/* Call this function to load GIO modules */
static void
gst_android_load_gio_modules (void)
{
  GTlsBackend *backend;
  const gchar *ca_certs;

   GST_G_IO_MODULE_LOAD(openssl);

  ca_certs = g_getenv ("CA_CERTIFICATES");

  backend = g_tls_backend_get_default ();
  if (backend && ca_certs) {
    GTlsDatabase *db;
    GError *error = NULL;

    db = g_tls_file_database_new (ca_certs, &error);
    if (db) {
      g_tls_backend_set_default_database (backend, db);
      g_object_unref (db);
    } else {
      g_warning ("Failed to create a database from file: %s",
          error ? error->message : "Unknown");
    }
  }
}

/* This is called by gst_init() */
void
gst_init_static_plugins (void)
{
   GST_PLUGIN_STATIC_REGISTER(coreelements);  GST_PLUGIN_STATIC_REGISTER(app);  GST_PLUGIN_STATIC_REGISTER(audioconvert);  GST_PLUGIN_STATIC_REGISTER(audiomixer);  GST_PLUGIN_STATIC_REGISTER(audiorate);  GST_PLUGIN_STATIC_REGISTER(audioresample);  GST_PLUGIN_STATIC_REGISTER(audiotestsrc);  GST_PLUGIN_STATIC_REGISTER(compositor);  GST_PLUGIN_STATIC_REGISTER(gio);  GST_PLUGIN_STATIC_REGISTER(rawparse);  GST_PLUGIN_STATIC_REGISTER(typefindfunctions);  GST_PLUGIN_STATIC_REGISTER(videoconvertscale);  GST_PLUGIN_STATIC_REGISTER(videorate);  GST_PLUGIN_STATIC_REGISTER(videotestsrc);  GST_PLUGIN_STATIC_REGISTER(volume);  GST_PLUGIN_STATIC_REGISTER(videofilter);  GST_PLUGIN_STATIC_REGISTER(deinterlace);  GST_PLUGIN_STATIC_REGISTER(videobox);  GST_PLUGIN_STATIC_REGISTER(videocrop);  GST_PLUGIN_STATIC_REGISTER(videomixer);  GST_PLUGIN_STATIC_REGISTER(playback);  GST_PLUGIN_STATIC_REGISTER(uriplaylistbin);  GST_PLUGIN_STATIC_REGISTER(tcp);  GST_PLUGIN_STATIC_REGISTER(rtsp);  GST_PLUGIN_STATIC_REGISTER(rtp);  GST_PLUGIN_STATIC_REGISTER(rtpmanager);  GST_PLUGIN_STATIC_REGISTER(udp);  GST_PLUGIN_STATIC_REGISTER(dtls);  GST_PLUGIN_STATIC_REGISTER(srtp);  GST_PLUGIN_STATIC_REGISTER(srt);  GST_PLUGIN_STATIC_REGISTER(webrtc);  GST_PLUGIN_STATIC_REGISTER(nice);  GST_PLUGIN_STATIC_REGISTER(rsrtp);  GST_PLUGIN_STATIC_REGISTER(rsrtsp);  GST_PLUGIN_STATIC_REGISTER(rswebrtc);
  gst_android_load_gio_modules ();
}
