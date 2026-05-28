# 03 — `MainActivity` lifecycle cleanup

**Priority:** Highest · **Effort:** Medium · **Estimated PR size:** ~90 LOC.

Pair every register/start/acquire in `MainActivity.onCreate` with a matching
unregister/quit/release in a new `onDestroy`. Normalise the mixed `captureLock`
usage to a single `lock()` / `try/finally / unlock()` shape.

| Sub-step | File                                                                                              | Topic                                                  |
|----------|---------------------------------------------------------------------------------------------------|--------------------------------------------------------|
| 03.1     | [03.1-report-finding.md](./03.1-report-finding.md)                                                | The report's complaint.                                 |
| 03.2     | [03.2-pre-state.md](./03.2-pre-state.md)                                                          | Every offending callsite, verbatim.                     |
| 03.3     | [03.3-ondestroy.md](./03.3-ondestroy.md)                                                          | New `onDestroy` method.                                  |
| 03.4     | [03.4-onstop.md](./03.4-onstop.md)                                                                | New `onStop` for projection callback.                    |
| 03.5     | [03.5-egl-release-helper.md](./03.5-egl-release-helper.md)                                        | Extracted `releaseEglResources()`.                       |
| 03.6     | [03.6-capturelock-normalization.md](./03.6-capturelock-normalization.md)                          | Replace `synchronized(captureLock)` with `lock/unlock`.  |
| 03.7     | [03.7-testing.md](./03.7-testing.md)                                                              | Test matrix.                                             |
| 03.8     | [03.8-rollback.md](./03.8-rollback.md)                                                            | Revert is purely additive — straightforward.             |

Depends on step 02 (the `CaptureResultBus.setListener` pairing).
