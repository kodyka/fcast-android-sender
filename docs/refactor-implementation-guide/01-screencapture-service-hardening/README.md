# 01 — Harden `ScreenCaptureService`

**Priority:** Highest · **Effort:** Low · **Estimated PR size:** ~40 LOC across 2 files.

Make the `ScreenCaptureService` start contract null-safe and non-sticky.

| Sub-step | File                                                                                  | Topic                                              |
|----------|---------------------------------------------------------------------------------------|----------------------------------------------------|
| 01.1     | [01.1-report-finding.md](./01.1-report-finding.md)                                    | The report's exact complaint.                       |
| 01.2     | [01.2-pre-state.md](./01.2-pre-state.md)                                              | The current Java source verbatim.                   |
| 01.3     | [01.3-add-action-constant.md](./01.3-add-action-constant.md)                          | Introduce `ACTION_RESULT`.                          |
| 01.4     | [01.4-null-safe-onstartcommand.md](./01.4-null-safe-onstartcommand.md)                | Rewrite `onStartCommand`.                           |
| 01.5     | [01.5-caller-update.md](./01.5-caller-update.md)                                      | `MainActivity` qualifies its `startForegroundService`. |
| 01.6     | [01.6-testing.md](./01.6-testing.md)                                                  | Manual + automated test matrix.                     |
| 01.7     | [01.7-rollback.md](./01.7-rollback.md)                                                | Single-line revert; optional partial rollback.      |

Apply sub-steps in numeric order. 01.3 and 01.4 must land together — splitting
them leaves the constant unused.
