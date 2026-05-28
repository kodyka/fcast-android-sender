# 02 — Remove `LocalBroadcastManager`

**Priority:** Highest · **Effort:** Medium · **Estimated PR size:** ~160 LOC across 3 files.

Replace the deprecated `LocalBroadcastManager` channel with a typed
in-process callback (`CaptureResultBus`) so the service and activity exchange
exactly one well-typed message.

| Sub-step | File                                                                                                  | Topic                                           |
|----------|-------------------------------------------------------------------------------------------------------|-------------------------------------------------|
| 02.1     | [02.1-report-finding.md](./02.1-report-finding.md)                                                    | The report's exact complaint.                    |
| 02.2     | [02.2-pre-state.md](./02.2-pre-state.md)                                                              | Current `LocalBroadcastManager` wiring.          |
| 02.3     | [02.3-capture-result-bus.md](./02.3-capture-result-bus.md)                                            | Full `CaptureResultBus.java` source.             |
| 02.4     | [02.4-service-update.md](./02.4-service-update.md)                                                    | Replace `sendBroadcast` with `deliver`.          |
| 02.5     | [02.5-activity-update.md](./02.5-activity-update.md)                                                  | Replace `BroadcastReceiver` with a lambda.       |
| 02.6     | [02.6-dependency-removal.md](./02.6-dependency-removal.md)                                            | Drop `androidx.localbroadcastmanager`.           |
| 02.7     | [02.7-testing.md](./02.7-testing.md)                                                                  | Test matrix.                                     |
| 02.8     | [02.8-rollback.md](./02.8-rollback.md)                                                                | Single-line revert; `CaptureResultBus` can stay. |

This step depends on **step 01** having landed (the action constant and
null-safe `onStartCommand` are already in place).
