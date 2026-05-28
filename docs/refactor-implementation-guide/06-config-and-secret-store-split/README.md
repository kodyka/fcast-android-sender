# 06 — Config and secret-store split

**Priority:** High · **Effort:** Medium · **Estimated PR size:** ~250 LOC.

Today the JSON config blob carries an `gstpop_api_key` field in plaintext.
Split secrets out into an `AndroidKeystore`-backed `SecretStore` (Kotlin)
and a thin Rust `SecretStore` trait that the runtimes look up keys through.

| Sub-step | File                                                                                                | Topic                                                |
|----------|-----------------------------------------------------------------------------------------------------|------------------------------------------------------|
| 06.1     | [06.1-report-finding.md](./06.1-report-finding.md)                                                  | Report quote.                                         |
| 06.2     | [06.2-pre-state.md](./06.2-pre-state.md)                                                            | Where `gstpop_api_key` is read today.                  |
| 06.3     | [06.3-config-shape.md](./06.3-config-shape.md)                                                      | New JSON shape with `secret_alias`.                    |
| 06.4     | [06.4-rust-secret-trait.md](./06.4-rust-secret-trait.md)                                            | Rust `SecretStore` trait + `InMemorySecretStore`.      |
| 06.5     | [06.5-rust-resolve-secret.md](./06.5-rust-resolve-secret.md)                                        | `resolve_secret(alias)` helper.                        |
| 06.6     | [06.6-kotlin-secret-store.md](./06.6-kotlin-secret-store.md)                                        | `SecretStore.kt` + `AndroidSecretStore.kt`.            |
| 06.7     | [06.7-jni-bridge.md](./06.7-jni-bridge.md)                                                          | JNI hand-off for `resolve_secret`.                     |
| 06.8     | [06.8-migration-path.md](./06.8-migration-path.md)                                                  | One-shot config-rewrite + alias generation.            |
| 06.9     | [06.9-testing.md](./06.9-testing.md)                                                                | Test matrix.                                            |
| 06.10    | [06.10-rollback.md](./06.10-rollback.md)                                                            | Per-layer rollback.                                     |

Depends on step 05 (the `App` context + `AppGraph` host the new traits).
