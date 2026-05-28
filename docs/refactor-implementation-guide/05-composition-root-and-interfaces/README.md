# 05 — Composition root & typed interfaces

**Priority:** High · **Effort:** Medium · **Estimated PR size:** ~300 LOC across Kotlin + Rust.

Replace static service-bridge call sites and the process-global
`BACKEND: Lazy<RwLock<…>>` with:

- A typed Kotlin `RuntimeBridge` interface + a JNI-backed implementation.
- A Kotlin composition root (`FcastApp` + `AppGraph`).
- A Rust `BackendRegistry` trait + a small `App` context struct.
- Explicit Slint-bridge wiring that goes through the new interfaces.

| Sub-step | File                                                                                                | Topic                                                  |
|----------|-----------------------------------------------------------------------------------------------------|--------------------------------------------------------|
| 05.1     | [05.1-report-finding.md](./05.1-report-finding.md)                                                  | The report's quote.                                     |
| 05.2     | [05.2-architecture-diagram.md](./05.2-architecture-diagram.md)                                      | Box-and-arrow diagram.                                  |
| 05.3     | [05.3-runtime-bridge-interface.md](./05.3-runtime-bridge-interface.md)                              | `RuntimeBridge.kt` + types.                              |
| 05.4     | [05.4-jni-runtime-bridge.md](./05.4-jni-runtime-bridge.md)                                          | `JniRuntimeBridge.kt`.                                   |
| 05.5     | [05.5-app-graph.md](./05.5-app-graph.md)                                                            | `AppGraph.kt` composition root.                          |
| 05.6     | [05.6-fcast-app.md](./05.6-fcast-app.md)                                                            | `FcastApp.kt` + manifest entry.                          |
| 05.7     | [05.7-rust-backend-registry.md](./05.7-rust-backend-registry.md)                                    | Rust `BackendRegistry` trait + impl.                     |
| 05.8     | [05.8-rust-app-context.md](./05.8-rust-app-context.md)                                              | Rust `App` struct + `OnceCell` bootstrap.                |
| 05.9     | [05.9-deprecate-globals.md](./05.9-deprecate-globals.md)                                            | `#[deprecated]` on the old `BACKEND` global.             |
| 05.10    | [05.10-slint-bridge-wiring.md](./05.10-slint-bridge-wiring.md)                                      | Slint global → `RuntimeBridge` route.                    |
| 05.11    | [05.11-testing.md](./05.11-testing.md)                                                              | Test matrix.                                             |
| 05.12    | [05.12-rollback.md](./05.12-rollback.md)                                                            | Per-layer rollback.                                      |

Depends on steps 01–04 having landed.
