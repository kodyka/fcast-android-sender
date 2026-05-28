# 08 — Split `MainActivity.java` & migrate the Android shell to Kotlin

**Priority:** Medium · **Effort:** High · **Estimated PR size:** ~300 LOC × 3 PRs.

Reduce `MainActivity.java` from 1158 LOC to a thin Kotlin shell
(target: ≤ 300 LOC), and migrate the boundary classes to Kotlin for
`lifecycleScope` / `StateFlow` / `kotlinx-coroutines-test` ergonomics.

| Sub-step | File                                                                                                | Topic                                                |
|----------|-----------------------------------------------------------------------------------------------------|------------------------------------------------------|
| 08.1     | [08.1-report-finding.md](./08.1-report-finding.md)                                                  | Report quote.                                         |
| 08.2     | [08.2-pre-state.md](./08.2-pre-state.md)                                                            | Current `MainActivity` responsibilities.              |
| 08.3     | [08.3-target-layout.md](./08.3-target-layout.md)                                                    | Final directory tree + responsibilities.              |
| 08.4     | [08.4-qr-scanner-launcher.md](./08.4-qr-scanner-launcher.md)                                        | Full `QrScannerLauncher.kt`.                          |
| 08.5     | [08.5-ui-state.md](./08.5-ui-state.md)                                                              | Full `UiState.kt` + Slint mapping.                    |
| 08.6     | [08.6-sender-controller.md](./08.6-sender-controller.md)                                            | Full `SenderController.kt`.                            |
| 08.7     | [08.7-mainactivity-kotlin.md](./08.7-mainactivity-kotlin.md)                                        | Full `MainActivity.kt`.                                |
| 08.8     | [08.8-slint-appstate-mapping.md](./08.8-slint-appstate-mapping.md)                                  | `UiState` → Slint `AppState` mapping (Rust side).      |
| 08.9     | [08.9-gradle-kotlin.md](./08.9-gradle-kotlin.md)                                                    | Gradle plugin & dependencies.                           |
| 08.10    | [08.10-migration-recipe.md](./08.10-migration-recipe.md)                                            | Three PRs explained.                                   |
| 08.11    | [08.11-testing.md](./08.11-testing.md)                                                              | Test matrix.                                           |
| 08.12    | [08.12-rollback.md](./08.12-rollback.md)                                                            | Per-PR rollback.                                       |

Depends on steps 02 (LBM removal), 04 (capture engine), 05 (`RuntimeBridge`),
07 (JNI split — symbols stable).
