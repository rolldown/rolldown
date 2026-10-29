const require = (await import('node:module')).createRequire(import.meta.url);
const assert = require('node:assert');

// Variants cover strict/allow-extension in both wrap-all and on-demand modes.
const variant = globalThis.__configName;

const lib = require('./dist/lib.js');
assert.strictEqual(lib.foo, 'foo_value');
assert.strictEqual(lib.bar, 42);

const main = require('./dist/main.js');
assert.strictEqual(main.foo, 'foo_value');
assert.strictEqual(main.bar, 42);

const runtimeHelpers = ['__esmMin', '__esm', '__toESM', '__commonJSMin'];

if (variant === 'allow-extension-on-demand') {
  // Keep the original positive control: this layout has a single valid runtime host, so allowing
  // signature extensions should merge the runtime into lib and expose at least one helper.
  assert.ok(
    runtimeHelpers.some((helper) => helper in lib),
    `${variant}: expected the runtime to merge into the lib entry chunk; exports were ${JSON.stringify(Object.keys(lib))}`,
  );
} else if (!variant.includes('allow-extension')) {
  // `strict` fixes lib's signature, so neither wrapping mode may leak runtime helpers. The default
  // `exports-only` behaves the same for an entry that declares exports. Other allow-extension
  // variants permit either runtime topology, so they only assert the declared exports above.
  for (const leaked of runtimeHelpers) {
    assert.ok(
      !(leaked in lib),
      `${variant}: lib signature leaked "${leaked}"; exports were ${JSON.stringify(Object.keys(lib))}`,
    );
  }
}
