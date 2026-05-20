# 1 · Decide the continuity contract

Before writing any code, pick exactly **one** row from this table. The
choice determines whether you build the whole service stack or just
refactor the Rust API.

| Continuity goal | Mechanism | Code reach |
|---|---|---|
| Survive config changes only (rotation, resize) | Already handled — `MainActivity` declares `configChanges="keyboardHidden|orientation|screenSize"`. **No service needed.** | Stop after step 2 (Rust API) + step 6 (probe tightening). |
| Survive activity backgrounded but process alive | A plain in-process `Service` (`startService`, not bound, no foreground) is sufficient. | Steps 2–6, but `GstPopService` is a bare `Service` and you skip the foreground / notification work in step 4. |
| Survive activity finish / task swipe-away | **Foreground service** required (Android 14+ enforces this for any long-running socket-holding work). | Full guide. |
| Survive process death | START_STICKY foreground + persisted config on disk; on relaunch, service restarts and re-binds 127.0.0.1:9000. | Full guide + `BackendLifecycle::autostart` extension in step 5. |

## Recommendation

Target row 3 (foreground service). It is the only configuration that
lets a localhost gst-pop daemon keep serving while the user is in
another app. Anything weaker than that gets killed by Android's
background execution limits on a real device within minutes.

If you decide row 1 is enough, **stop reading**: step 2 alone
(explicit Rust start/stop API) plus step 6 (remove implicit
probe-start) covers it, and no Java/service code is needed.

## Reality check: do you actually need cross-app continuity?

The honest answer for most FCast sender use cases is **no**. The user
opens the app, picks a receiver, casts something, and the app is in
the foreground for the entire session. If that's your usage profile,
row 1 is what you ship.

Row 3 starts to matter when:

- You want the receiver dashboard to stay reachable while the user
  uses the phone normally between cast sessions.
- You are reusing the same localhost daemon for a screen-recording or
  background-audio scenario (i.e., something started from a quick
  setting / tile rather than from the launcher).
- You're shipping a debug build that wants the daemon up for
  long-running connectivity tests.

If none of those apply, treat the rest of the guide as a *capability
spec* — implement step 2 + step 6 now, and keep the rest as an
on-demand follow-up.

## What this guide assumes from here on

Row 3 / row 4. The steps still apply if you weaken to row 2 — just
skip the foreground notification and manifest `foregroundServiceType`
parts.

Next: [02-rust-daemon-api.md](./02-rust-daemon-api.md).
