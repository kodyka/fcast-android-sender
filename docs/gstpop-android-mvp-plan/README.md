# gstpop-runtime Android-first migration — step index

Per-step detail files for the plan in
[`../gstpop-android-mvp-plan.md`](../gstpop-android-mvp-plan.md).

Execute strictly in order. Each step links its predecessors.

## Phase 1 — Android MVP (highest priority)

1. [Step 1 — `EmbeddedConfig` + `start_embedded_with_config`](./step-01-embedded-config.md)
2. [Step 2 — Preserve vendored `TcpListener` pre-bind](./step-02-preserve-prebind.md)
3. [Step 3 — Typed client helpers](./step-03-typed-client.md)
4. [Step 4 — Android-safe media path handling](./step-04-android-safe-media.md)
5. [Step 5 — Embedded-server integration tests](./step-05-integration-tests.md)
6. [Step 6 — Android arm64 build validation](./step-06-android-arm64-build.md)

## Phase 2 — Android polish (medium priority)

7. [Step 7 — Optional JNI bridge](./step-07-jni-bridge.md)
8. [Step 8 — Media discovery wrapper](./step-08-media-discovery.md)
9. [Step 9 — Typed protocol enums](./step-09-typed-protocol.md)

## Phase 3 — Desktop & cross-platform (low priority)

10. [Step 10 — Desktop tooling feature](./step-10-desktop-tools.md)
11. [Step 11 — Multi-ABI Android](./step-11-multi-abi-android.md)
12. [Step 12 — Separate desktop CLI crate](./step-12-desktop-cli-crate.md)
13. [Step 13 — Signal handling (CLI only)](./step-13-signal-handling.md)
