# 04 — Extract a `CaptureEngine`

**Priority:** Highest · **Effort:** High · **Estimated PR size:** 300–500 LOC across 3 PRs.

Move the entire screen-capture pipeline (MediaProjection, EGL/OpenGL, frame
loop, throttling) out of `MainActivity` and into two dedicated Kotlin classes:

- `ScreenCaptureCoordinator` — owns the lifecycle, holds the
  `MediaProjectionManager`, listens to `CaptureResultBus`, drives `CaptureEngine`.
- `CaptureEngine` — owns the EGL context, GL textures/FBOs, frame loop, and
  the `nativeProcessFrame` hop.

| Sub-step | File                                                                                              | Topic                                              |
|----------|---------------------------------------------------------------------------------------------------|----------------------------------------------------|
| 04.1     | [04.1-report-finding.md](./04.1-report-finding.md)                                                | Report's quote.                                     |
| 04.2     | [04.2-target-layout.md](./04.2-target-layout.md)                                                  | New `capture/` directory tree.                       |
| 04.3     | [04.3-capture-config.md](./04.3-capture-config.md)                                                | Full `CaptureConfig.kt`.                             |
| 04.4     | [04.4-capture-permission-result.md](./04.4-capture-permission-result.md)                          | Full `CapturePermissionResult.kt`.                   |
| 04.5     | [04.5-screen-capture-coordinator.md](./04.5-screen-capture-coordinator.md)                        | Full `ScreenCaptureCoordinator.kt`.                  |
| 04.6     | [04.6-capture-engine.md](./04.6-capture-engine.md)                                                | Full `CaptureEngine.kt` (skeleton).                  |
| 04.7     | [04.7-mainactivity-removal.md](./04.7-mainactivity-removal.md)                                    | Strip the extracted code from `MainActivity`.        |
| 04.8     | [04.8-migration-recipe.md](./04.8-migration-recipe.md)                                            | Six-move recipe + PR breakdown.                      |
| 04.9     | [04.9-testing.md](./04.9-testing.md)                                                              | Test matrix and benchmarks.                          |
| 04.10    | [04.10-rollback.md](./04.10-rollback.md)                                                          | Per-PR rollback strategy.                            |

Depends on steps 01, 02, 03 having landed. Land 04 in 3 sub-PRs (see 04.8) —
each one independently shippable.
