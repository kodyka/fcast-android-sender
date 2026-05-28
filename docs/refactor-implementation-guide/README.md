# Refactor Implementation Guide — fcast-android-sender

> Source-of-truth: the deep-research report attached by the requester (see `99-references.md`).
> This is a **guide-only** plan — no source files are modified in this PR.

This directory turns the [deep-research refactor plan](./99-references.md) into a series of
concrete, reviewable, code-level steps. Each step lists what to change, why (with the
report's finding), the current state in `main`, the target state, and a verified diff /
snippet that a developer can apply.

## Why this guide exists

The research report identified three classes of problem:

1. **Boundary fragility** — `ScreenCaptureService` and the two backend-hosting services
   (`GstPopService`, `MigrationRuntimeService`) have unsafe or deprecated contracts.
2. **Architectural concentration** — `MainActivity.java` (1158 lines on `main`) and
   `src/lib.rs` (3076 lines on `main`) collapse multiple layers into single files.
3. **Toolchain & process drift** — Gradle 8.9 / AGP 8.7-line / compileSdk 34, plus a
   duplicated GitLab release pipeline that produces an unsigned APK.

The guide is structured so you can land each step as an isolated PR (≤ ~300 LOC where
practical), in priority order, with rollback notes.

## Priority order (from the report)

| # | File                                                          | Priority   | Effort |
|---|---------------------------------------------------------------|------------|--------|
| 0 | [00-baseline-and-scope.md](./00-baseline-and-scope.md)        | —          | —      |
| 1 | [01-screencapture-service-hardening.md](./01-screencapture-service-hardening.md) | Highest | Low    |
| 2 | [02-remove-localbroadcastmanager.md](./02-remove-localbroadcastmanager.md)       | Highest | Medium |
| 3 | [03-mainactivity-lifecycle-cleanup.md](./03-mainactivity-lifecycle-cleanup.md)   | Highest | Medium |
| 4 | [04-capture-engine-extraction.md](./04-capture-engine-extraction.md)             | Highest | High   |
| 5 | [05-composition-root-and-interfaces.md](./05-composition-root-and-interfaces.md) | High    | Medium |
| 6 | [06-config-and-secret-store-split.md](./06-config-and-secret-store-split.md)     | High    | Medium |
| 7 | [07-split-src-lib-rs.md](./07-split-src-lib-rs.md)                               | High    | High   |
| 8 | [08-mainactivity-split-and-kotlin-shell.md](./08-mainactivity-split-and-kotlin-shell.md) | Medium | High |
| 9 | [09-ci-cd-consolidation.md](./09-ci-cd-consolidation.md)       | Medium     | Medium |
| 10 | [10-android-tests.md](./10-android-tests.md)                  | Medium     | Medium |
| 11 | [11-build-stack-upgrade.md](./11-build-stack-upgrade.md)      | Medium     | High   |
| 12 | [12-performance-pass.md](./12-performance-pass.md)            | Later      | Medium |
| 13 | [13-rollout-and-rollback.md](./13-rollout-and-rollback.md)    | —          | —      |
| 99 | [99-references.md](./99-references.md)                        | —          | —      |

## How to use this guide

- Each numbered file is a **single PR's worth of work**. The steps are deliberately
  ordered so that earlier PRs do not depend on the design of later PRs.
- All code blocks in this guide are **verified against `main`** at the time of writing.
  Line numbers, identifiers, and signatures will drift; treat them as anchors, not
  invariants. When applying a step, re-grep for the patterns.
- "Before / After" diffs use unified-diff syntax so they paste cleanly into a feature
  branch. They are not committed by this PR.
- Rollback notes are mandatory for every step. If a step lacks one, treat the step as
  not ready to land.

## What this guide is *not*

- It is not an exhaustive design document. The report already covers that.
- It is not a rewrite plan. The report explicitly recommends incremental refactor.
- It does not propose dropping the Slint UI or the Rust runtime — the hybrid
  architecture stays. The boundary between the two layers is what is being cleaned up.
