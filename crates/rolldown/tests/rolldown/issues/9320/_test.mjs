import path from 'node:path';
import { pathToFileURL } from 'node:url';
import assert from 'node:assert';

// https://github.com/rolldown/rolldown/issues/9320
//
// `form.js` and `action.js` are both dynamic-import targets and form a
// static-import cycle. Before the fix, `try_insert_into_existing_chunk`
// (via `find_merge_target`) would asymmetrically merge the cycle into one
// of the dynamic-entry chunks. That chunk file is what
// `import('./<entry>.js')` resolves to at runtime, so the *other* module's
// named exports leaked into the dynamic-import namespace observed by
// callers.
//
// The bundle's `main` runs `import('./form.js')` and `import('./action.js')`
// inside an async function (no top-level await — TLA disables the chunk
// optimization that triggered the bug). It stashes the resolved namespaces
// on `globalThis`, then this harness verifies each one exposes exactly the
// exports declared by its source module.

const distDir = path.join(import.meta.dirname, 'dist');
await import(pathToFileURL(path.join(distDir, 'main.js')).href);
await globalThis.__9320_done;

assert.deepStrictEqual(
  Object.keys(globalThis.__9320_formNs).sort(),
  ['callActionFromForm', 'formImpl'],
  'form namespace must expose only form.js exports',
);
assert.deepStrictEqual(
  Object.keys(globalThis.__9320_actionNs).sort(),
  ['actionImpl', 'callFormFromAction'],
  'action namespace must expose only action.js exports',
);
