# Refactor Implementation Guide — fcast-android-sender

> Source-of-truth: the deep-research report attached by the requester (see [`99-references.md`](./99-references.md)).
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

## Priority order

Steps 00 – 11 are split into per-concern sub-step files (one directory each, with a
`README.md` that lists the sub-steps and their topics). Steps 12, 13 and 99 remain as
single files.

| #  | Step                                                                                | Priority | Effort | Layout            |
|----|-------------------------------------------------------------------------------------|----------|--------|-------------------|
| 0  | [00-baseline-and-scope/](./00-baseline-and-scope/)                                  | —        | —      | Split             |
| 1  | [01-screencapture-service-hardening/](./01-screencapture-service-hardening/)         | Highest  | Low    | Split             |
| 2  | [02-remove-localbroadcastmanager/](./02-remove-localbroadcastmanager/)               | Highest  | Medium | Split             |
| 3  | [03-mainactivity-lifecycle-cleanup/](./03-mainactivity-lifecycle-cleanup/)            | Highest  | Medium | Split             |
| 4  | [04-capture-engine-extraction/](./04-capture-engine-extraction/)                      | Highest  | High   | Split             |
| 5  | [05-composition-root-and-interfaces/](./05-composition-root-and-interfaces/)          | High     | Medium | Split (incl. Slint) |
| 6  | [06-config-and-secret-store-split/](./06-config-and-secret-store-split/)              | High     | Medium | Split             |
| 7  | [07-split-src-lib-rs/](./07-split-src-lib-rs/)                                        | High     | High   | Split (PRs 7.1–7.7) |
| 8  | [08-mainactivity-split-and-kotlin-shell/](./08-mainactivity-split-and-kotlin-shell/)  | Medium   | High   | Split (incl. Slint) |
| 9  | [09-ci-cd-consolidation/](./09-ci-cd-consolidation/)                                  | Medium   | Medium | Split (PRs 9.A–9.F) |
| 10 | [10-android-tests/](./10-android-tests/)                                              | Medium   | Medium | Split             |
| 11 | [11-build-stack-upgrade/](./11-build-stack-upgrade/)                                  | Medium   | High   | Split (PRs 11.1–11.7) |
| 12 | [12-performance-pass.md](./12-performance-pass.md)                                    | Later    | Medium | Single file       |
| 13 | [13-rollout-and-rollback.md](./13-rollout-and-rollback.md)                            | —        | —      | Single file       |
| 99 | [99-references.md](./99-references.md)                                                | —        | —      | Single file       |

## Per-step sub-files

Each split step directory follows the same five-section structure:

1. **Report finding** — direct quote from the deep-research report.
2. **Pre-state** — verified file content / line counts on `main`.
3. **Changes** — one sub-step per concern, each with full code examples (no
   cross-references to "see snippet below").
4. **Testing** — per-PR test matrix and pass criteria.
5. **Rollback** — per-PR revert instructions, including partial rollback paths.

Open the `README.md` inside any step directory for the sub-step table.

## How to use this guide

- Each numbered step is a **single PR's worth of work** (or, for split steps, one PR per
  sub-step). The steps are deliberately ordered so that earlier PRs do not depend on the
  design of later PRs.
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
