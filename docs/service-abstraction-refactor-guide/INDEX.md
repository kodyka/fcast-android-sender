# Service Abstraction & SRT Overlay Refactor — Implementation Guide

Step-by-step guide for making Android services optional, creating an
independent UI layer, and adding image-overlay capabilities to SRT video
sources.

## STEP files

| # | Title | File |
|---|-------|------|
| 01 | [Service Manager Trait](./STEP-01-service-manager-trait.md)          | `src/service/mod.rs` (new) |
| 02 | [GstPopService Refactor](./STEP-02-gstpop-service-refactor.md)      | `src/backend/gstpop/service.rs` (modify) |
| 03 | [Migration Service Wrapper](./STEP-03-migration-service-wrapper.md)  | `src/migration/service.rs` (new) |
| 04 | [Service Configuration Storage](./STEP-04-service-config-storage.md) | `src/backend/persistence.rs` (extend) |
| 05 | [UI Service Config Page](./STEP-05-ui-service-config-page.md)        | `ui/pages/service_config_page.slint` (new) |
| 06 | [UI Service Bridge](./STEP-06-ui-service-bridge.md)                  | `ui/state/service_bridge.slint` (new) |
| 07 | [Refactor Media Backend Page](./STEP-07-refactor-media-backend-page.md) | `ui/pages/media_backend_page.slint` (modify) |
| 08 | [Main UI Decoupling](./STEP-08-main-ui-decoupling.md)                | `ui/main.slint` (modify) |
| 09 | [SRT Source Manager](./STEP-09-srt-source-manager.md)                | `src/srt/mod.rs` (new) |
| 10 | [Image Overlay System](./STEP-10-image-overlay-system.md)            | `src/overlay/mod.rs` (new) |
| 11 | [Mixer Overlay Integration](./STEP-11-mixer-overlay-integration.md)  | `src/migration/nodes/mixer.rs` (extend) |
| 12 | [SRT & Overlay UI](./STEP-12-srt-overlay-ui.md)                      | `ui/pages/srt_config_page.slint` (new) |
| 13 | [Module Reorganisation](./STEP-13-module-reorganisation.md)          | `src/lib.rs` (restructure) |
| 14 | [Error Handling](./STEP-14-error-handling.md)                        | cross-cutting |
| 15 | [Testing Infrastructure](./STEP-15-testing-infrastructure.md)        | `tests/` (new) |
| 16 | [Documentation & Examples](./STEP-16-documentation-examples.md)      | `docs/`, `README.md` |

## Conventions used in this guide

* **Rust snippets** are self-contained; copy-paste should compile against
  the existing `Cargo.toml` dependencies.
* **Slint snippets** follow the repo's theme-token and `@tr()` conventions.
  No raw hex colours or hard-coded `font-size: Npx` values.
* Each STEP lists the files to create / modify, the Rust trait or struct
  signatures involved, and a "Wire-up checklist" that ties the new code
  into the existing codebase.
* The guide does **not** modify code — it is reference-only.
