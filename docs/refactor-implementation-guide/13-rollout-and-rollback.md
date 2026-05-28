# 13 — Rollout, phased timeline, and rollback strategy

**Priority:** —

This is not a code step. It is the project plan that wires steps 01–12 together
into a sequence that survives partial landings, rollbacks, and unrelated parallel
work.

## Source of truth

The phased timeline below is the report's "Incremental migration timeline"
gantt translated into PR-shaped phases and tied to the per-step files in this
directory.

> "First stabilise lifecycle and service behaviour, then extract Android
> boundary abstractions, then move configuration and backend orchestration
> behind interfaces with manual constructor injection, then split the largest
> files into Android and Rust submodules, and only then perform the platform/
> toolchain upgrade. That sequence will reduce both technical risk and
> regression risk."

— `deep-research-report-3.md`, "Executive summary".

## Phase plan

### Phase A — Stabilise the current shell (Highest priority)

| Step | File                                                                                  | Estimated PRs |
|------|---------------------------------------------------------------------------------------|---------------|
| 01   | [01-screencapture-service-hardening.md](./01-screencapture-service-hardening.md)      | 1             |
| 02   | [02-remove-localbroadcastmanager.md](./02-remove-localbroadcastmanager.md)            | 1             |
| 03   | [03-mainactivity-lifecycle-cleanup.md](./03-mainactivity-lifecycle-cleanup.md)        | 1             |

**Phase exit criteria:**

- Capture, stop, reconnect happy paths verified manually on at least two devices.
- `:app:testDebugUnitTest` (added in step 10) is green.
- No `LocalBroadcastManager` import remains under `app/src/main`.
- `MainActivity` has explicit `onDestroy` that pairs every register in `onCreate`.

### Phase B — Extract boundaries (Highest / High priority)

| Step | File                                                                                    | Estimated PRs |
|------|-----------------------------------------------------------------------------------------|---------------|
| 04   | [04-capture-engine-extraction.md](./04-capture-engine-extraction.md)                    | 3             |
| 05   | [05-composition-root-and-interfaces.md](./05-composition-root-and-interfaces.md)        | 1–2           |
| 06   | [06-config-and-secret-store-split.md](./06-config-and-secret-store-split.md)            | 1             |

**Phase exit criteria:**

- `MainActivity.java` size drops to ≤ 700 LOC (from 1158).
- `ScreenCaptureCoordinator.startCapture` is the only entry point in code into
  the capture engine.
- `RuntimeBridge` is the only path to the static service-bridge calls.
- `backend.json` no longer contains `gstpop_api_key` on any device that has
  launched the new build at least once.

### Phase C — Modularise (High / Medium priority)

| Step | File                                                                                          | Estimated PRs |
|------|-----------------------------------------------------------------------------------------------|---------------|
| 07   | [07-split-src-lib-rs.md](./07-split-src-lib-rs.md)                                            | 5–7           |
| 08   | [08-mainactivity-split-and-kotlin-shell.md](./08-mainactivity-split-and-kotlin-shell.md)      | 3             |

**Phase exit criteria:**

- `src/lib.rs` ≤ 400 LOC.
- `MainActivity` size ≤ 400 LOC, in Kotlin.
- `nm -D` symbol set on `libfcastsender.so` is byte-identical to the pre-phase
  baseline.

### Phase D — Upgrade and harden (Medium priority)

| Step | File                                                                                    | Estimated PRs |
|------|-----------------------------------------------------------------------------------------|---------------|
| 09   | [09-ci-cd-consolidation.md](./09-ci-cd-consolidation.md)                                | 1             |
| 10   | [10-android-tests.md](./10-android-tests.md)                                            | 1–2           |
| 11   | [11-build-stack-upgrade.md](./11-build-stack-upgrade.md)                                | 5–7           |

**Phase exit criteria:**

- Signed release APK published from GitHub Actions.
- `:app:connectedDebugAndroidTest` green in CI.
- AGP 9.x, Gradle 9.x, `targetSdk` 36, NDK r28c — or each item documented as
  intentionally held back with a tracking issue.

### Phase E — Optional optimisation (Later priority)

| Step | File                                                                | Estimated PRs |
|------|---------------------------------------------------------------------|---------------|
| 12   | [12-performance-pass.md](./12-performance-pass.md)                  | profile-driven|

## Rollback matrix

Per-area rollback strategy, derived from the report's "Risk and rollback" table.

| Area                                           | Step(s)       | Rollback playbook                                                                                                                            |
|------------------------------------------------|---------------|----------------------------------------------------------------------------------------------------------------------------------------------|
| Screen capture regressions                     | 01, 03, 04    | Re-enable legacy coordinator behind a `Capture.NEW_PIPELINE` flag. Keep the service contract unchanged for one release.                       |
| Cross-process communication                    | 02            | Restore `LocalBroadcastManager` paths. The `CaptureResultBus` class can stay in the tree, unused.                                             |
| JNI / lifetime regressions                     | 07            | Restore the old `lib.rs` composition while preserving every exported symbol. Symbol-stability gate in step 07 prevents this from being silent.|
| Backend selection regressions                  | 05            | Flip composition root back to `backend::current()`; keep the new interfaces compiled but unused.                                              |
| Storage migration regressions                  | 06            | Dual-read old JSON and new store for one release; only delete the inline key after a successful read/write parity check.                      |
| Toolchain upgrade breakage                     | 11            | Revert the specific sub-PR (Gradle, AGP, SDK, NDK, or Kotlin) — they are each independent.                                                    |
| Release pipeline breakage                      | 09            | The frozen GitLab pipeline (`when: manual`) is the rollback. Two successful GitHub releases must ship before the GitLab pipeline can be deleted. |

## Cross-cutting risks

- **Parallel work on `MainActivity.java`** — Phase A and Phase B all touch this
  file. Coordinate with any concurrent feature branches; rebase rather than
  merge when possible to keep the diff readable.
- **`src/lib.rs` rebase pain** — Phase C step 07 is the only step that does
  large file moves on the Rust side. Land it in a sprint when no other large
  Rust PRs are in flight.
- **GStreamer Android upgrade is not in scope** — every step assumes 1.28.0.
  Treat any GStreamer bump as a separate effort with its own validation matrix.

## Communication checklist

For each phase exit:

- Post a short note in the release channel summarising what changed for
  contributors (e.g. "MainActivity is now Kotlin; rebase your branches off
  `main` to pick up the rename").
- Update the project README's "Architecture overview" section to point at this
  guide.
- Re-record the "developer setup" recording if step 11 changes the Gradle or
  NDK versions visible to developers.

## When to stop

This guide is finished when:

1. `wc -l` on `MainActivity.java` (or `.kt`) is below the project's chosen
   ceiling (≤ 400 LOC suggested).
2. `wc -l` on `src/lib.rs` is below 400 LOC.
3. `backend.json` no longer carries secret material.
4. CI passes signed releases from `main` without manual GitLab steps.

Anything beyond that is a future plan.
