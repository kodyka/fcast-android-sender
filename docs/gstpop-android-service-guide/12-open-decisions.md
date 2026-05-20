# 12 · Open decisions

Things you should consciously pick before merging the implementation
PR. Each has a recommended default; the rationale matters more than
the answer.

## 12.1 Foreground service type

**Options:** `dataSync`, `mediaPlayback`, `specialUse`.

| Type | Pros | Cons |
|---|---|---|
| `dataSync` | Defensible match for a long-running local server. No extra runtime UI hooks. | Generic — users may wonder what the notification is for. |
| `mediaPlayback` | Honest about the daemon's eventual purpose. Lets Android render media controls in the notification shade. | The daemon doesn't actually expose play/pause to the system — the media-style notification controls would be lies. Requires `FOREGROUND_SERVICE_MEDIA_PLAYBACK` permission and tighter Android policy enforcement. |
| `specialUse` | Catch-all for anything not covered by the typed list. | Requires `<property android:name="android.app.PROPERTY_SPECIAL_USE_FGS_SUBTYPE" android:value="…" />` and a clear declaration of intent. Play Store will scrutinise this on submission. |

**Recommendation:** `dataSync`. It's the most accurate technical
match for "long-running localhost socket" and avoids both the
media-controls lie and the Play Store friction.

## 12.2 Binder API vs status polling

**Options:** `null` `onBind` + 1Hz Rust poller, **or** `Messenger` /
AIDL push.

| Approach | Pros | Cons |
|---|---|---|
| Polling (default) | Zero IPC surface area. UI ↔ Rust statics are in-process. | 1Hz tick wastes a few µs of CPU per second; UI can lag by up to 1s on rapid state transitions. |
| `Messenger` | Push updates — UI sees Starting → Running within milliseconds. | New IPC layer to maintain. Messenger callbacks marshal through binder, which means another thread context to reason about. |
| AIDL | Strongly typed RPC. Useful if you ever want a third-party app to query gst-pop status. | Heaviest option; overkill unless you need cross-app callers. |

**Recommendation:** stick with polling. The 1Hz cost is negligible and
the state transitions humans actually notice (Starting → Ready) are
on the order of 100–500ms, well within poll tolerance.

If you do switch to `Messenger` later, the binder lives in
`GstPopService.onBind`. The bridge class stays the same — UI just
calls `bind` instead of `queryStatus`.

## 12.3 API key in `EXTRA_CONFIG_JSON`

`StoredBackendConfig` round-trips `gstpop_api_key`. Passing it through
the Intent extra means:

- It appears in `dumpsys activity services org.fcast.android.sender`.
- It's logged by `Log.d(TAG, "onStartCommand …")` if you print the
  intent.

Mitigations:

1. **Don't log the intent extras.** Replace
   `Log.d(TAG, "onStartCommand intent=" + intent)` with action-only
   logging.
2. **Pass only the URL/port via Intent.** Read the API key from disk
   inside `nativeStart` — `StoredBackendConfig::load(files_dir)` is
   already cheap and gives you the canonical source.
3. **Don't worry about it.** The API key is for connecting *to* the
   local daemon; if an attacker can `dumpsys` your services they have
   adb shell anyway and the localhost daemon is the least of your
   worries.

**Recommendation:** Option 2 if you care about defence in depth.
Option 3 if pragmatism wins.

## 12.4 `START_STICKY` vs `START_REDELIVER_INTENT`

| Mode | Behaviour after OS kill |
|---|---|
| `START_STICKY` | OS restarts service with `intent = null`. Step 4.1's null-intent branch self-stops. **No auto-start of the daemon.** |
| `START_REDELIVER_INTENT` | OS redelivers the last explicit `ACTION_START` intent. `nativeStart` runs again **without** the user re-requesting it. |

**Recommendation:** `START_STICKY` (the current default). Redelivery
means "if I die, please come back exactly how I was" — but for a
foreground notification, that surprises the user. Let the explicit
`BackendLifecycle::autostart` path on activity recreate be the only
auto-restart vector.

## 12.5 `android:stopWithTask`

`stopWithTask="false"` (default in step 4.2) means swiping the app
away does **not** kill the service. That's what makes "daemon
survives task swipe" actually work.

If you flip it to `"true"`, task swipe = daemon dies. Useful only if
you actively *don't* want the daemon to survive a swipe-away.

**Recommendation:** `false` for row-3/row-4 continuity contracts.
`true` only if you decided continuity row 2 in step 1.

## 12.6 Same process vs separate process

Step 4.3 forbids `android:process="…"` on the `<service>` block. The
case for a separate process would be: isolate gst-pop crashes from
the UI. The case against: another `loadLibrary`, another tokio
runtime, double the memory footprint, IPC for every status query.

**Recommendation:** same process. The daemon is reliable enough that
crash isolation isn't worth the cost.

## 12.7 Allow remote callers to start the service

`android:exported="false"` (default) restricts service starts to this
app's own UID. If you ever want an automation app (Tasker, etc.) to
start gst-pop via Intent, you'd set `exported="true"` + a permission
guard. None of this is in scope today.

**Recommendation:** keep `exported="false"`. Revisit only if a
concrete external-caller use case appears.

## 12.8 Daemon port

Default 9000 is hard-coded in `StoredBackendConfig::defaults` and the
service's `parse_config_port` fallback. If you want to make this
user-tunable:

- The Slint UI already has `gstpop-url` as an editable field
  (`ui/bridge.slint:294`). The user can change the port via the URL
  today.
- `start_embedded` already takes `port: u16` — no struct changes
  needed.
- `request_service_start` already passes the full config to the
  service, so the new port flows through.

**No work needed** — the design is already port-configurable.

## 12.9 Multiple simultaneous gst-pop instances

Not supported. The atomics in `embedded.rs` enforce a single
in-process daemon, and `start_embedded(p1)` followed by
`start_embedded(p2)` does port-switch semantics (§8.4), not
multi-instance.

If you ever need two daemons (e.g., one for casting, one for
recording), the design has to change substantially:

- `HANDLE: Mutex<HashMap<u16, ServerHandle>>`
- `STATE: HashMap<u16, EmbeddedState>`
- The bridge needs a port argument on stop.

**Recommendation:** don't. Use a single daemon and route multiple
pipelines through it (gst-pop is designed for this).

## 12.10 Whether to keep this guide checked in

Three options:

1. Keep it under `docs/` (current PR).
2. Move to a wiki or external doc.
3. Implement the work, then prune the guide.

**Recommendation:** keep in `docs/`. The guide is also a design
record — it documents *why* the lifecycle looks the way it does. After
implementation, the per-step files can stay as a reference (and shrink
in size as items get implemented).

Next: [13-file-change-list.md](./13-file-change-list.md).
