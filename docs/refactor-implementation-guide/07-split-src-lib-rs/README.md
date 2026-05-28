# 07 — Split `src/lib.rs`

**Priority:** High · **Effort:** High · **Estimated PR size:** ~250 LOC × 7 PRs.

Carve up the 3076-line `src/lib.rs` into focused modules while keeping every
exported `Java_org_fcast_android_sender_*` symbol exactly where it is on the
ABI surface.

| Sub-step | File                                                                                                | Topic                                                   |
|----------|-----------------------------------------------------------------------------------------------------|---------------------------------------------------------|
| 07.1     | [07.1-report-finding.md](./07.1-report-finding.md)                                                  | Report quote + 3076-line baseline.                       |
| 07.2     | [07.2-target-module-tree.md](./07.2-target-module-tree.md)                                          | Final layout of `src/`.                                  |
| 07.3     | [07.3-symbol-inventory.md](./07.3-symbol-inventory.md)                                              | Every `Java_…` symbol on `main`.                          |
| 07.4     | [07.4-pr-7.1-helpers.md](./07.4-pr-7.1-helpers.md)                                                  | **PR 7.1** — mechanical move of helpers.                 |
| 07.5     | [07.5-pr-7.2-jni-split.md](./07.5-pr-7.2-jni-split.md)                                              | **PR 7.2** — split JNI symbols by callsite.              |
| 07.6     | [07.6-pr-7.3-application.md](./07.6-pr-7.3-application.md)                                          | **PR 7.3** — carve up `Application`.                      |
| 07.7     | [07.7-pr-7.4-android-main.md](./07.7-pr-7.4-android-main.md)                                        | **PR 7.4** — move `android_main`.                         |
| 07.8     | [07.8-pr-7.5-inline-tests.md](./07.8-pr-7.5-inline-tests.md)                                        | **PR 7.5** — relocate inline tests.                       |
| 07.9     | [07.9-pr-7.6-crate-promotion.md](./07.9-pr-7.6-crate-promotion.md)                                  | **PR 7.6** — promote modules to crates.                   |
| 07.10    | [07.10-pr-7.7-raw-pointer-audit.md](./07.10-pr-7.7-raw-pointer-audit.md)                            | **PR 7.7** — safety audit (`from_raw` etc.).              |
| 07.11    | [07.11-testing.md](./07.11-testing.md)                                                              | Test matrix + symbol-stability check.                     |
| 07.12    | [07.12-rollback.md](./07.12-rollback.md)                                                            | Per-PR rollback.                                          |

Depends on steps 05 and 06 (the `app::App` context + secret store live in
the post-split layout).
