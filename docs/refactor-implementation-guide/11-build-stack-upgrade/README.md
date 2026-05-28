# 11 — Build-stack upgrade

**Priority:** Medium · **Effort:** High · **Estimated PR size:** ~50 LOC × 7 PRs + bug-fix follow-ups.

Bring the Gradle wrapper, AGP, NDK, compileSdk/targetSdk, and Kotlin
baseline forward in controlled increments. Verify native GStreamer
compatibility at each step.

| Sub-step | File                                                                                             | Topic                                                            |
|----------|--------------------------------------------------------------------------------------------------|------------------------------------------------------------------|
| 11.1     | [11.1-report-finding.md](./11.1-report-finding.md)                                               | Report quotes.                                                    |
| 11.2     | [11.2-pre-state.md](./11.2-pre-state.md)                                                         | Current versions on `main`.                                         |
| 11.3     | [11.3-target-state.md](./11.3-target-state.md)                                                   | Final versions.                                                     |
| 11.4     | [11.4-risk-register.md](./11.4-risk-register.md)                                                 | Per-risk mitigations.                                                |
| 11.5     | [11.5-pr-11.1-gradle-wrapper.md](./11.5-pr-11.1-gradle-wrapper.md)                                | **PR 11.1** — Gradle 8.9 → 9.5.                                       |
| 11.6     | [11.6-pr-11.2-agp-8.13.md](./11.6-pr-11.2-agp-8.13.md)                                            | **PR 11.2** — AGP 8.7 → 8.13.                                        |
| 11.7     | [11.7-pr-11.3-compilesdk-35.md](./11.7-pr-11.3-compilesdk-35.md)                                  | **PR 11.3** — compileSdk 34 → 35.                                     |
| 11.8     | [11.8-pr-11.4-agp-9.md](./11.8-pr-11.4-agp-9.md)                                                  | **PR 11.4** — AGP 8.13 → 9.0.                                          |
| 11.9     | [11.9-pr-11.5-targetsdk-36.md](./11.9-pr-11.5-targetsdk-36.md)                                    | **PR 11.5** — targetSdk 34 → 36 (Android 16).                          |
| 11.10    | [11.10-pr-11.6-kotlin-2.3.md](./11.10-pr-11.6-kotlin-2.3.md)                                       | **PR 11.6** — Kotlin baseline 2.3.                                      |
| 11.11    | [11.11-pr-11.7-ndk-r28c.md](./11.11-pr-11.7-ndk-r28c.md)                                            | **PR 11.7** — NDK r25c → r28c.                                          |
| 11.12    | [11.12-testing.md](./11.12-testing.md)                                                             | Test matrix per sub-PR.                                               |
| 11.13    | [11.13-rollback.md](./11.13-rollback.md)                                                            | Per-PR rollback.                                                       |
