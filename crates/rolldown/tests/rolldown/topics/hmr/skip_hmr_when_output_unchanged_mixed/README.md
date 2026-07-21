# skip_hmr_when_output_unchanged_mixed

Companion to `skip_hmr_when_output_unchanged` ([rolldown#10024](https://github.com/rolldown/rolldown/issues/10024)). Covers the two paths the base fixture cannot: a **mixed save** (one no-op file plus one really-changed file in the same watch event) and a **no-op edit of a non-self-accepting module** whose update would otherwise propagate to its importer.

`hub.js` self-accepts and imports `a.js` and `b.js` (both non-self-accepting). Each module counts its runs in a global.

| Step | Edit                                                      | Update type | Re-runs    |
| ---- | --------------------------------------------------------- | ----------- | ---------- |
| 0    | `a.js` re-saved identical (no-op) + `b.js` really changed | `patch`     | `b`, `hub` |
| 1    | `a.js` whitespace-only change                             | `noop`      | nothing    |
| 2    | `a.js` really changed (control)                           | `patch`     | `a`, `hub` |

Step 0 must ship only `b.js` (plus the `hub.js` boundary): `a.js` must not appear in the changed ids, so `a` never re-runs. Step 1 would have re-run `a` and `hub` before suppression existed; now nothing re-runs. The `beforeExit` guard in `main.js` asserts the exact run counts (`a: 2, b: 2, hub: 3`) and fails loudly if a suppressed step ships or a shipped step suppresses.
