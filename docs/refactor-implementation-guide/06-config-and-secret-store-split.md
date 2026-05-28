# 06 — Split backend config storage from secrets

**Priority:** High · **Effort:** Medium · **Estimated PR size:** ~200 LOC.

## Goal

Stop serialising `gstpop_api_key` into plain JSON inside the app files directory.
Keep non-secret backend config in a small typed store (DataStore-style or a
versioned JSON file is fine), and move any secret/API-key material into Android's
encrypted credential storage with a stable *alias* that the config can reference.

## Report finding

> "`src/backend/persistence.rs` defines `StoredBackendConfig` with
> `gstpop_api_key: Option<String>` and serialises the structure directly to
> `backend.json`. That is fine for non-secret defaults, but it is not a good
> long-term place for credentials."

— `deep-research-report-3.md`, "Detailed findings".

> "Store non-secret backend config in DataStore-style persisted settings; move
> any secret/API-key material to a non-plain-text store; stop serialising secrets
> into `backend.json`."

— same document, "Refactor plan".

## Pre-state on `main`

`src/backend/persistence.rs` (42 LOC, verbatim except trimmed comments):

```rust
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct StoredBackendConfig {
    pub kind: BackendKind,
    pub gstpop_url: String,
    pub gstpop_api_key: Option<String>,   // ← the offending field
    pub gstpop_pipeline_id: String,
}

impl StoredBackendConfig {
    pub fn defaults() -> Self { /* … */ }
    pub fn load(files_dir: &Path) -> Result<Self> { /* reads backend.json */ }
    pub fn save(&self, files_dir: &Path) -> Result<()> { /* writes backend.json */ }
}
```

`backend.json` is written to the app's files directory in plain JSON. There is
no key derivation, no Android Keystore involvement, no migration path.

## Target shape

```
backend.json (plain)            ← URL, pipeline id, alias reference
    └─ "gstpop_api_key_alias": "default_gstpop_key"

EncryptedSharedPreferences      ← actual secret material
    └─ key = "default_gstpop_key", value = <api key bytes>
```

The Rust side never sees the secret bytes directly; it asks for them through a
`SecretStore` trait that the Android shell implements with
`EncryptedSharedPreferences` + Android Keystore.

## Rust changes

### `StoredBackendConfig` becomes secret-free

```rust
// src/backend/persistence.rs
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct StoredBackendConfig {
    pub kind: BackendKind,
    pub gstpop_url: String,
    pub gstpop_pipeline_id: String,
    /// Stable reference into the platform secret store.
    /// `None` means "no api-key configured".
    pub gstpop_api_key_alias: Option<String>,

    /// Migration field — read once if present, then cleared on next save.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gstpop_api_key: Option<String>,
}

impl StoredBackendConfig {
    pub fn defaults() -> Self {
        Self {
            kind: BackendKind::Migration,
            gstpop_url: "ws://127.0.0.1:9000".into(),
            gstpop_pipeline_id: "0".into(),
            gstpop_api_key_alias: None,
            gstpop_api_key: None,
        }
    }
    // load / save unchanged
}
```

### `SecretStore` trait

```rust
// src/backend/secret_store.rs   (NEW)
use anyhow::Result;

pub trait SecretStore: Send + Sync {
    fn get(&self, alias: &str) -> Result<Option<String>>;
    fn put(&self, alias: &str, value: &str) -> Result<()>;
    fn delete(&self, alias: &str) -> Result<()>;
}

/// A no-op implementation for host tests.
pub struct InMemorySecretStore { inner: parking_lot::Mutex<std::collections::HashMap<String, String>> }
impl InMemorySecretStore {
    pub fn new() -> Self { Self { inner: Default::default() } }
}
impl SecretStore for InMemorySecretStore {
    fn get(&self, alias: &str) -> Result<Option<String>> { Ok(self.inner.lock().get(alias).cloned()) }
    fn put(&self, alias: &str, value: &str) -> Result<()> { self.inner.lock().insert(alias.to_string(), value.to_string()); Ok(()) }
    fn delete(&self, alias: &str) -> Result<()> { self.inner.lock().remove(alias); Ok(()) }
}
```

### One-shot migration on load

```rust
// src/backend/persistence.rs (add helper)
pub fn migrate_inline_secret(
    cfg: &mut StoredBackendConfig,
    store: &dyn SecretStore,
) -> Result<bool> {
    let Some(plain) = cfg.gstpop_api_key.take() else { return Ok(false); };
    let alias = cfg.gstpop_api_key_alias
        .clone()
        .unwrap_or_else(|| "default_gstpop_key".to_string());
    store.put(&alias, &plain)?;
    cfg.gstpop_api_key_alias = Some(alias);
    Ok(true)
}
```

Call sites:

```rust
let mut cfg = StoredBackendConfig::load(&files_dir)?;
let migrated = migrate_inline_secret(&mut cfg, &*secret_store)?;
if migrated { cfg.save(&files_dir)?; }
```

After one successful save, `gstpop_api_key` is gone from disk forever — the
`skip_serializing_if = "Option::is_none"` attribute ensures the field is not
re-introduced into the JSON on subsequent writes.

## Android implementation of `SecretStore`

### Kotlin facade

```kotlin
// app/src/main/java/org/fcast/android/sender/data/AndroidSecretStore.kt   (NEW)
package org.fcast.android.sender.data

import android.content.Context
import androidx.security.crypto.EncryptedSharedPreferences
import androidx.security.crypto.MasterKey

class AndroidSecretStore(context: Context) {
    private val masterKey = MasterKey.Builder(context)
        .setKeyScheme(MasterKey.KeyScheme.AES256_GCM)
        .build()

    private val prefs = EncryptedSharedPreferences.create(
        context,
        "fcast_secrets",
        masterKey,
        EncryptedSharedPreferences.PrefKeyEncryptionScheme.AES256_SIV,
        EncryptedSharedPreferences.PrefValueEncryptionScheme.AES256_GCM,
    )

    fun get(alias: String): String? = prefs.getString(alias, null)
    fun put(alias: String, value: String) = prefs.edit().putString(alias, value).apply()
    fun delete(alias: String)             = prefs.edit().remove(alias).apply()
}
```

### Wire it through JNI

Add three Rust JNI functions (purely for the secret store) that delegate into
Kotlin. The recommended shape:

```rust
// src/lib.rs   (NEW exported symbols)
#[unsafe(no_mangle)]
pub extern "C" fn Java_org_fcast_android_sender_data_RustSecretStore_nativeRegister(
    env: jni::JNIEnv,
    _class: jni::objects::JClass,
    java_store: jni::objects::JObject,
) { /* store the global reference in OnceCell, used by JniSecretStore::get/put/delete */ }
```

`JniSecretStore` (Rust impl of `SecretStore`) holds the `GlobalRef` to the
Kotlin `AndroidSecretStore` and invokes `get`/`put`/`delete` via JNI. The
implementation is mechanical — mirror the existing `GstPopServiceBridge` JNI
wiring pattern in `src/lib.rs`.

### Dependency

`app/build.gradle` — add `androidx.security:security-crypto`:

```diff
 dependencies {
+    implementation "androidx.security:security-crypto:1.1.0-alpha06"
 }
```

`security-crypto` is the official Jetpack library for EncryptedSharedPreferences /
EncryptedFile + Android Keystore-backed master keys. There is no widely-used
stable alternative for this use case — pin the version you adopt and update with
Jetpack releases.

## Behaviour matrix

| Existing `backend.json` state                             | After step 06 first launch                                          |
|-----------------------------------------------------------|----------------------------------------------------------------------|
| `gstpop_api_key` absent                                    | `gstpop_api_key_alias` stays `None`. No migration.                  |
| `gstpop_api_key = Some("…")`                               | Secret copied to EncryptedSharedPreferences under `default_gstpop_key`; field removed from JSON on next save. |
| `gstpop_api_key_alias = Some("foo")` and `gstpop_api_key = Some("…")` | Secret copied under alias `foo`; field removed.                     |
| Both `Some(_)` and alias already populated in EncryptedShared… | Alias wins; the plaintext field is discarded.                       |

## Testing

| Test                                                              | How                                                                      |
|-------------------------------------------------------------------|--------------------------------------------------------------------------|
| Rust round-trip                                                    | `cargo test -p fcastsender backend::persistence`.                        |
| Migration test                                                     | New Rust unit test using `InMemorySecretStore`; assert post-migration JSON does not contain `gstpop_api_key`. |
| Encrypted storage works on emulator                                | `:app:connectedAndroidTest` writes/reads via `AndroidSecretStore`.       |
| Existing UI/launcher flow                                          | Manual: launch app on a device with an existing `backend.json` carrying an inline key. Confirm capture still works. |
| Lint                                                               | `./gradlew :app:lint`.                                                   |

## Rollback

Two-step:

- Rust: revert `persistence.rs` to read the `gstpop_api_key` field again. The
  field is `Option<String>` and `serde(default)`, so an already-migrated JSON
  (no field) still parses.
- Android: keep `EncryptedSharedPreferences`. Even if the Rust side stops using
  it, the secret material stays accessible until the user explicitly clears app
  data. Do **not** delete the prefs file as part of rollback.

## Follow-ups (not in this PR)

- Add a UI affordance to wipe the secret store (Settings → "Clear API key").
- Introduce a typed config repository on the Android side (`BackendConfigRepository`)
  that calls into Rust through the `RuntimeBridge` rather than reading
  `backend.json` directly — alongside step 08's UI work.
