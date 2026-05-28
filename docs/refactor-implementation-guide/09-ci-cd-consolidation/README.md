# 09 — CI / CD consolidation

**Priority:** Medium · **Effort:** Medium · **Estimated PR size:** ~150 LOC YAML.

Consolidate the release pipeline on GitHub Actions, add SDK / NDK /
GStreamer / Cargo caching, add release signing, and freeze (but keep) the
GitLab pipeline as a documented fallback.

| Sub-step | File                                                                            | Topic                                                    |
|----------|---------------------------------------------------------------------------------|----------------------------------------------------------|
| 09.1     | [09.1-report-finding.md](./09.1-report-finding.md)                              | Report quote.                                              |
| 09.2     | [09.2-pre-state.md](./09.2-pre-state.md)                                        | Existing pipelines on `main`.                              |
| 09.3     | [09.3-target-state.md](./09.3-target-state.md)                                  | Final workflow tree.                                       |
| 09.4     | [09.4-add-caching.md](./09.4-add-caching.md)                                    | Caching for composite setup action.                        |
| 09.5     | [09.5-rename-debug.md](./09.5-rename-debug.md)                                  | Rename `android-release-apk.yml` → `android-debug-apk.yml`.|
| 09.6     | [09.6-add-release-pipeline.md](./09.6-add-release-pipeline.md)                  | New signed-release workflow.                               |
| 09.7     | [09.7-gradle-signing-config.md](./09.7-gradle-signing-config.md)                | `app/build.gradle` signingConfig.                          |
| 09.8     | [09.8-freeze-gitlab.md](./09.8-freeze-gitlab.md)                                | Freeze banner on `.gitlab-ci.yml`.                          |
| 09.9     | [09.9-github-secrets.md](./09.9-github-secrets.md)                              | Keystore secret provisioning.                              |
| 09.10    | [09.10-symbol-stability-ci.md](./09.10-symbol-stability-ci.md)                  | CI gate for step 07's symbol-diff guard.                    |
| 09.11    | [09.11-testing.md](./09.11-testing.md)                                          | Test matrix.                                                |
| 09.12    | [09.12-rollback.md](./09.12-rollback.md)                                         | Rollback.                                                   |
