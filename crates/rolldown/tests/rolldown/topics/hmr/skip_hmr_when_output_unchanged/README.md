# skip_hmr_when_output_unchanged

Reproduction for the Full Bundle Mode (FBM) item in
[rolldown#10019](https://github.com/rolldown/rolldown/issues/10019):

> **Skip HMR / reload when the output doesn't change** (should be relatively easy in FBM).
> Vite currently always triggers HMR/reload; with full-bundle mode this should be
> relatively easy to implement.

## What it shows

`mod.js` is a self-accepting module. The three HMR steps edit it as follows:

| Step | Edit                                                         | Rendered patch output          |
| ---- | ------------------------------------------------------------ | ------------------------------ |
| 0    | re-save `mod.js` with identical content (a no-op save)       | identical to the loaded module |
| 1    | whitespace-only change (inter-statement blank lines removed) | **byte-identical to step 0**   |
| 2    | real change (`'hello'` → `'world'`)                          | different (control)            |

Look at `artifacts.snap`: **steps 0 and 1 both emit `update type: patch` with byte-identical
`## Code`** even though neither edit changed the bundled output (the HMR printer re-prints
the AST, so source whitespace never reaches the output). Today FBM re-renders and ships a
patch for any change that maps to a module — the only early-out is `HmrUpdate::Noop` when a
changed file maps to _zero_ modules (`crates/rolldown/src/hmr/hmr_stage.rs`,
`changed_modules.is_empty()`); there is no comparison of the newly-rendered module code
against its previous version.

Step 2 is the control: a genuine change produces a different patch, confirming HMR itself
works.

## Expected behavior once the optimization lands

Steps 0 and 1 should become `update type: noop` (no patch sent, no client-side re-run),
while step 2 stays a `patch`. When that is implemented, update this snapshot accordingly.
