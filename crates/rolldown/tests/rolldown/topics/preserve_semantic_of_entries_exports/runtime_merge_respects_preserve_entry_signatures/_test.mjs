const require = (await import('node:module')).createRequire(import.meta.url);
const assert = require('node:assert');

// Variants: undefined (the base config, `strict`) and 'allow-extension'.
const variant = globalThis.__configName ?? 'strict';

const lib = require('./dist/lib.js');
assert.strictEqual(lib.foo, 'foo_value');
assert.strictEqual(lib.bar, 42);

const main = require('./dist/main.js');
assert.strictEqual(main.foo, 'foo_value');
assert.strictEqual(main.bar, 42);

const runtimeHelpers = ['__esmMin', '__esm', '__toESM', '__commonJSMin'];

if (variant.includes('allow-extension')) {
  // Extension is permitted, so the runtime is allowed to merge into the `lib`
  // entry chunk; its helpers then appear as extension exports on lib.
  assert.ok(
    runtimeHelpers.some((helper) => helper in lib),
    `${variant}: expected the runtime to merge into the lib entry chunk; exports were ${JSON.stringify(Object.keys(lib))}`,
  );
} else {
  // `strict` (the base config) fixes lib's signature, so the guard keeps the
  // runtime — and its helpers — out of lib. (The default `exports-only` behaves
  // the same for an entry that declares exports; that path is exercised by the
  // broader entry-exports snapshots.)
  //
  // NOTE: `init_lib` (lib's own ESM wrapper, present because `strictExecutionOrder`
  // wraps the module) is still re-exported because `main` triggers lib's init
  // cross-chunk. That is a separate facade/chunking concern, tracked apart from the
  // runtime-merge guard, so it is intentionally not asserted here.
  for (const leaked of runtimeHelpers) {
    assert.ok(
      !(leaked in lib),
      `${variant}: lib signature leaked "${leaked}"; exports were ${JSON.stringify(Object.keys(lib))}`,
    );
  }
}
