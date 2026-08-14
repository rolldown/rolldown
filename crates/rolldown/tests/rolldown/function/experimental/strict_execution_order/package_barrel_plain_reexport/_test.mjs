import assert from 'node:assert';

globalThis.__events = [];

const page = await import('./dist/main.js');
const loaded = await page.loadPage();

// The reader imported two side-effect-free components through a package barrel that plain-imports
// and re-exports them, and reads them only now in a deferred "render". Under strict execution order
// the barrel's `init_*` must forward to each component's `init_*` at init time. The regression left
// the components' `init_*` with zero call sites, so they were still `undefined` at render and
// `render()` came back `NO_CHECKBOX|NO_RADIO`.
const result = loaded.render();
assert.strictEqual(
  result,
  'Checkbox|Radio',
  `barrel must forward init to the plain-imported, re-exported components before the deferred render reads them; got ${JSON.stringify(result)} events=${JSON.stringify(globalThis.__events)}`,
);
