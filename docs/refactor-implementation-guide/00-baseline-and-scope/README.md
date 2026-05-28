# 00 — Baseline & Scope

Ground truth that every later step refers back to.

| Sub-step | File                                                        | Topic                                                  |
|----------|-------------------------------------------------------------|--------------------------------------------------------|
| 00.1     | [00.1-line-counts.md](./00.1-line-counts.md)                | Verified file sizes on `main`.                          |
| 00.2     | [00.2-confirmed-findings.md](./00.2-confirmed-findings.md)  | Report findings cross-checked against `main`.           |
| 00.3     | [00.3-line-number-drift.md](./00.3-line-number-drift.md)    | Report claims with bad line refs, needing re-grep.      |
| 00.4     | [00.4-in-scope.md](./00.4-in-scope.md)                      | Which files this refactor touches.                      |
| 00.5     | [00.5-out-of-scope.md](./00.5-out-of-scope.md)              | Which files it does **not** touch.                      |
| 00.6     | [00.6-conventions.md](./00.6-conventions.md)                | "Pre-state / Target / Diff / Testing / Rollback" layout.|

Read every sub-file before applying any later step. The five-section layout in
00.6 is enforced for every per-step file.
