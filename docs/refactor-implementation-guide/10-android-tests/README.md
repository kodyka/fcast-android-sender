# 10 — Android-side automated tests

**Priority:** Medium · **Effort:** Medium · **Estimated PR size:** ~250 LOC tests + workflow changes.

Three layers of automated tests on the Android side:

1. **JVM unit tests** for service contracts (action handling, null-intent
   paths, notification building) and parser logic (status-JSON parsing).
2. **Instrumentation tests** for foreground-service start contracts and
   `RuntimeBridge` round-trip on an emulator.
3. **Re-affirm the Rust-side headless Slint UI tests** as the required
   pre-merge gate, and drop `--test-threads=1` once step 07 isolates the
   gst-pop global state.

| Sub-step | File                                                                                              | Topic                                                       |
|----------|---------------------------------------------------------------------------------------------------|-------------------------------------------------------------|
| 10.1     | [10.1-report-finding.md](./10.1-report-finding.md)                                                | Report quote.                                                |
| 10.2     | [10.2-pre-state.md](./10.2-pre-state.md)                                                          | Existing test surface.                                       |
| 10.3     | [10.3-test-layout.md](./10.3-test-layout.md)                                                      | Final test directory tree.                                    |
| 10.4     | [10.4-jvm-screencapture-test.md](./10.4-jvm-screencapture-test.md)                                | `ScreenCaptureServiceTest.java` (full).                       |
| 10.5     | [10.5-jvm-status-parser-test.md](./10.5-jvm-status-parser-test.md)                                | `StatusParserTest.kt` (full).                                  |
| 10.6     | [10.6-jvm-sender-controller-test.md](./10.6-jvm-sender-controller-test.md)                        | `SenderControllerTest.kt` (full).                              |
| 10.7     | [10.7-instrumented-runtime-bridge-test.md](./10.7-instrumented-runtime-bridge-test.md)            | `RuntimeBridgeInstrumentedTest.kt` (full).                     |
| 10.8     | [10.8-instrumented-screencapture-test.md](./10.8-instrumented-screencapture-test.md)              | `ScreenCaptureServiceInstrumentedTest.kt` (full).               |
| 10.9     | [10.9-gradle-test-dependencies.md](./10.9-gradle-test-dependencies.md)                            | Gradle test dependency additions.                              |
| 10.10    | [10.10-ci-jvm-job.md](./10.10-ci-jvm-job.md)                                                       | CI step for `:app:testDebugUnitTest`.                          |
| 10.11    | [10.11-ci-instrumented-job.md](./10.11-ci-instrumented-job.md)                                     | CI step for connected android tests.                            |
| 10.12    | [10.12-drop-test-threads-gate.md](./10.12-drop-test-threads-gate.md)                                | Remove `--test-threads=1` after step 07.5.                       |
| 10.13    | [10.13-testing.md](./10.13-testing.md)                                                              | Test matrix.                                                    |
| 10.14    | [10.14-rollback.md](./10.14-rollback.md)                                                            | Rollback.                                                       |
