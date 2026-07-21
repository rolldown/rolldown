# skip_hmr_when_output_unchanged

Covers the Full Bundle Mode (FBM) item from [rolldown#10024](https://github.com/rolldown/rolldown/issues/10024) (tracker: [rolldown#10019](https://github.com/rolldown/rolldown/issues/10019)):

> **Skip HMR / reload when the output doesn't change.**

`mod.js` is a self-accepting module. The three HMR steps edit it as follows:

| Step | Edit                                                         | Update type |
| ---- | ------------------------------------------------------------ | ----------- |
| 0    | re-save `mod.js` with identical content (a no-op save)       | `noop`      |
| 1    | whitespace-only change (inter-statement blank lines removed) | `noop`      |
| 2    | real change (`'hello'` → `'world'`)                          | `patch`     |

After a changed module is re-scanned, the HMR stage renders its new code and compares it against the same render of the previous code (`crates/rolldown/src/hmr/hmr_stage.rs`). A byte-identical render means the edit cannot affect the client — the printer re-prints the AST, so source whitespace and comments never reach the output — and the module is dropped from the update. When nothing is left to ship, the client gets `update type: noop`: no patch is sent and no module re-runs.

Step 2 is the control: a real change still ships a patch.

The accept callback in `mod.js` asserts `mod.value === 'world'`. The callback registered by one run of the module fires at the next shipped update, with that update's exports. Steps 0 and 1 ship nothing, so the callback fires exactly once — at step 2. If a no-op step regressed into shipping a patch, the callback would fire early with `'hello'` and execution would fail.
