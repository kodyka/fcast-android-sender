# 05 — Explicit composition root + typed boundary interfaces

**Priority:** High · **Effort:** Medium · **Estimated PR size:** ~250 LOC.

## Goal

Replace static service-bridge calls (`GstPopServiceBridge.start/stop/queryStatus`,
`MigrationRuntimeServiceBridge.start/stop/queryStatus`) and the process-global
backend (`BACKEND: Lazy<RwLock<Arc<dyn MediaBackend>>>`) with **explicit
constructor-injected facades** at the Android boundary and **trait-backed
repositories** on the Rust side.

This is the single biggest "soft" win in the whole refactor: it shrinks the blast
radius of every step that follows.

## Report finding

> "Rust backend selection is global via `BACKEND: Lazy<RwLock<Arc<dyn MediaBackend>>>`,
> and Android service orchestration is exposed through static Java bridge methods
> such as `GstPopServiceBridge.start/stop/queryStatus()` and
> `MigrationRuntimeServiceBridge.start/stop/queryStatus()`. This makes testing and
> replacement harder, because the composition root is not explicit and state is
> process-global."

— `deep-research-report-3.md`, "Detailed findings" and "Refactor plan".

> "I would **not** introduce a heavy DI framework as the first move. The nearer-
> term win is to replace global state and static bridge calls with explicit
> constructors and interfaces."

— same document, "Proposed target architecture and libraries".

## Pre-state on `main`

`src/backend/mod.rs:30-39`:

```rust
static BACKEND: once_cell::sync::Lazy<RwLock<Arc<dyn MediaBackend>>> =
    once_cell::sync::Lazy::new(|| RwLock::new(Arc::new(MigrationBackend::new())));

pub fn current() -> Arc<dyn MediaBackend> {
    BACKEND.read().clone()
}

pub fn install(new_backend: Arc<dyn MediaBackend>) {
    *BACKEND.write() = new_backend;
}
```

Static Java bridges (verified by `wc -l`): `GstPopServiceBridge.java` is 70 LOC,
`MigrationRuntimeServiceBridge.java` is 73 LOC. Both expose `start(Context, …)`,
`stop(Context)`, `queryStatus()`, plus three `private static native` declarations.

## Target architecture

```
┌─────────────────────────┐         ┌────────────────────────────┐
│ MainActivity / Shell    │ owns →  │ AppGraph (composition root)│
└─────────────────────────┘         └─────────────┬──────────────┘
                                                  │ constructs
                                                  ▼
                                    ┌────────────────────────────┐
                                    │ RuntimeBridge              │   ← interface
                                    │   startEmbeddedBackend()   │
                                    │   stopEmbeddedBackend()    │
                                    │   graphCommand(action, p)  │
                                    │   backendStatus()          │
                                    └─────────────┬──────────────┘
                                                  │ implements
                                                  ▼
                          ┌────────────────────────────────────────────────┐
                          │ JniRuntimeBridge — delegates to                │
                          │ GstPopServiceBridge / MigrationRuntimeServiceBridge │
                          └────────────────────────────────────────────────┘
```

## Android side — typed boundary

### `RuntimeBridge` interface

```kotlin
// app/src/main/java/org/fcast/android/sender/runtime/RuntimeBridge.kt   (NEW)
package org.fcast.android.sender.runtime

import org.json.JSONObject

enum class BackendKind { MIGRATION, GSTPOP }

data class BackendStatus(val state: String, val message: String?)

interface RuntimeBridge {
    suspend fun startEmbeddedBackend(kind: BackendKind, configJson: String): BackendStatus
    suspend fun stopEmbeddedBackend(kind: BackendKind): BackendStatus
    suspend fun backendStatus(kind: BackendKind): BackendStatus
    suspend fun graphCommand(action: String, params: JSONObject = JSONObject()): JSONObject
}
```

### `JniRuntimeBridge` — single concrete implementation

```kotlin
// app/src/main/java/org/fcast/android/sender/runtime/JniRuntimeBridge.kt   (NEW)
package org.fcast.android.sender.runtime

import android.content.Context
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import org.fcast.android.sender.GstPopServiceBridge
import org.fcast.android.sender.MigrationRuntimeServiceBridge
import org.json.JSONObject

class JniRuntimeBridge(private val appContext: Context) : RuntimeBridge {

    override suspend fun startEmbeddedBackend(
        kind: BackendKind, configJson: String,
    ): BackendStatus = withContext(Dispatchers.IO) {
        val json = when (kind) {
            BackendKind.GSTPOP -> {
                GstPopServiceBridge.start(appContext, configJson)
                GstPopServiceBridge.queryStatus()
            }
            BackendKind.MIGRATION -> {
                MigrationRuntimeServiceBridge.start(appContext, configJson)
                MigrationRuntimeServiceBridge.queryStatus()
            }
        }
        parseStatus(json)
    }

    override suspend fun stopEmbeddedBackend(kind: BackendKind): BackendStatus = withContext(Dispatchers.IO) {
        val json = when (kind) {
            BackendKind.GSTPOP -> { GstPopServiceBridge.stop(appContext); GstPopServiceBridge.queryStatus() }
            BackendKind.MIGRATION -> { MigrationRuntimeServiceBridge.stop(appContext); MigrationRuntimeServiceBridge.queryStatus() }
        }
        parseStatus(json)
    }

    override suspend fun backendStatus(kind: BackendKind): BackendStatus = withContext(Dispatchers.IO) {
        val json = when (kind) {
            BackendKind.GSTPOP -> GstPopServiceBridge.queryStatus()
            BackendKind.MIGRATION -> MigrationRuntimeServiceBridge.queryStatus()
        }
        parseStatus(json)
    }

    override suspend fun graphCommand(action: String, params: JSONObject): JSONObject =
        withContext(Dispatchers.IO) {
            val cmd = JSONObject().put(action, params)
            val resp = nativeHandleGraphCommand(cmd.toString())
            JSONObject(resp)
        }

    private fun parseStatus(raw: String): BackendStatus {
        val obj = runCatching { JSONObject(raw) }.getOrNull()
            ?: return BackendStatus("error", "unparseable status: $raw")
        return BackendStatus(
            state = obj.optString("state", "unknown"),
            message = obj.optString("message").takeIf { it.isNotBlank() },
        )
    }

    private external fun nativeHandleGraphCommand(json: String): String
}
```

The new `nativeHandleGraphCommand` symbol wraps the existing
`try_handle_command_json` Rust path. Adding it does **not** mean removing the
static service bridges — those keep working until step 07 lands a Rust-side
re-org. This step is about the Kotlin/Java surface, not the JNI surface.

### `AppGraph` — the single composition root

```kotlin
// app/src/main/java/org/fcast/android/sender/AppGraph.kt   (NEW)
package org.fcast.android.sender

import android.content.Context
import org.fcast.android.sender.capture.CaptureEngine
import org.fcast.android.sender.capture.ScreenCaptureCoordinator
import org.fcast.android.sender.runtime.JniRuntimeBridge
import org.fcast.android.sender.runtime.RuntimeBridge

class AppGraph(appContext: Context) {
    val runtime: RuntimeBridge = JniRuntimeBridge(appContext.applicationContext)
    val captureEngine = CaptureEngine(onFrame = { /* forward to runtime */ })
    val captureCoordinator = ScreenCaptureCoordinator(appContext.applicationContext, captureEngine)
}
```

And a tiny `Application` subclass to host it:

```kotlin
// app/src/main/java/org/fcast/android/sender/FcastApp.kt   (NEW)
package org.fcast.android.sender

import android.app.Application

class FcastApp : Application() {
    lateinit var graph: AppGraph
        private set

    override fun onCreate() {
        super.onCreate()
        graph = AppGraph(this)
    }
}
```

`AndroidManifest.xml` change:

```diff
-<application android:label="@string/app_name" …>
+<application
+    android:name=".FcastApp"
+    android:label="@string/app_name" …>
```

`MainActivity` now reaches its dependencies through `((FcastApp) getApplication()).graph`.

### Keep the static bridges as a thin shim

Do **not** delete `GstPopServiceBridge` / `MigrationRuntimeServiceBridge` in this PR.
They still own the JNI declarations and are referenced from `src/lib.rs`. The
guarantee this step makes is only that *new* code goes through `RuntimeBridge`.

## Rust side — trait-backed selection (no global mutex)

### Today

```rust
static BACKEND: once_cell::sync::Lazy<RwLock<Arc<dyn MediaBackend>>> =
    once_cell::sync::Lazy::new(|| RwLock::new(Arc::new(MigrationBackend::new())));
```

### Target — eliminate global, take by reference

```rust
// src/backend/registry.rs   (NEW)
use std::sync::Arc;

use crate::backend::{BackendKind, MediaBackend};

pub trait BackendRegistry: Send + Sync {
    fn install(&self, backend: Arc<dyn MediaBackend>);
    fn current(&self) -> Arc<dyn MediaBackend>;
}

/// Thread-safe in-memory implementation. Created once per process by the
/// composition root (JNI bootstrap on Android, main() on host).
pub struct InMemoryRegistry {
    inner: parking_lot::RwLock<Arc<dyn MediaBackend>>,
}

impl InMemoryRegistry {
    pub fn new(initial: Arc<dyn MediaBackend>) -> Self {
        Self { inner: parking_lot::RwLock::new(initial) }
    }
}

impl BackendRegistry for InMemoryRegistry {
    fn install(&self, backend: Arc<dyn MediaBackend>) { *self.inner.write() = backend; }
    fn current(&self) -> Arc<dyn MediaBackend> { self.inner.read().clone() }
}
```

### Migration: introduce an injected `App` context

```rust
// src/app.rs   (NEW)
use std::sync::Arc;

use crate::backend::registry::{BackendRegistry, InMemoryRegistry};
use crate::backend::MigrationBackend;

pub struct App {
    pub backends: Arc<dyn BackendRegistry>,
}

impl App {
    pub fn new() -> Self {
        let registry = InMemoryRegistry::new(Arc::new(MigrationBackend::new()));
        Self { backends: Arc::new(registry) }
    }
}
```

Hold the `App` for the JNI side in a `OnceCell<App>` created during
`Java_org_fcast_android_sender_MainActivity_nativeInit`. **Crucially**, this
`OnceCell` only carries the bootstrap context — every function below takes the
relevant `Arc<dyn BackendRegistry>` by reference so unit tests can pass a fake.

```rust
// src/lib.rs   (excerpt — the bootstrap path)
static APP: once_cell::sync::OnceCell<App> = once_cell::sync::OnceCell::new();

fn app() -> &'static App {
    APP.get_or_init(App::new)
}
```

### Deprecate the old globals

```diff
- pub fn current() -> Arc<dyn MediaBackend> {
-     BACKEND.read().clone()
- }
- pub fn install(new_backend: Arc<dyn MediaBackend>) {
-     *BACKEND.write() = new_backend;
- }
+ #[deprecated = "use App::backends instead (see src/app.rs)"]
+ pub fn current() -> Arc<dyn MediaBackend> {
+     crate::app::app().backends.current()
+ }
+ #[deprecated = "use App::backends instead (see src/app.rs)"]
+ pub fn install(new_backend: Arc<dyn MediaBackend>) {
+     crate::app::app().backends.install(new_backend);
+ }
```

Keep the deprecated functions for one release so step 07 can move call sites
without blocking on this step.

## Testing

| Test                                                              | How                                                                      |
|-------------------------------------------------------------------|--------------------------------------------------------------------------|
| Existing Slint headless UI tests pass                             | `cargo test -p fcastsender --test ui_snapshots`.                         |
| Existing Rust backend tests pass                                  | `cargo test -p fcastsender`.                                              |
| `RuntimeBridge` JVM test                                          | New `:app:testDebugUnitTest` with a fake `RuntimeBridge` that returns canned JSON; assert that the UI state holder maps `state=error` to the error banner. |
| Static bridges still compile                                       | The PR does not remove them — `./gradlew :app:assembleDebug` must succeed. |
| Lint                                                              | `./gradlew :app:lint`.                                                   |

## Rollback

Three layers, three rollback levels:

- Revert the Kotlin files (`AppGraph`, `FcastApp`, `JniRuntimeBridge`, `RuntimeBridge`).
  `AndroidManifest.xml` reverts the `android:name=".FcastApp"` line.
- Revert `src/app.rs` and `src/backend/registry.rs`. Keep the `#[deprecated]`
  attributes off the old globals for one more release if needed.
- The static service bridges (`GstPopServiceBridge`, `MigrationRuntimeServiceBridge`)
  are untouched.

## Follow-ups (not in this PR)

- Split `backend.json` and move secrets to a real store — **Step 06**.
- Split `src/lib.rs` along the `App` boundary — **Step 07**.
- Move `MainActivity` to Kotlin, replacing direct dependency lookups with
  `application.graph.runtime` — **Step 08**.
