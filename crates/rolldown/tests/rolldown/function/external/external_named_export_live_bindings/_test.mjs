import assert from 'node:assert/strict';
import https from 'node:https';
import { createRequire, syncBuiltinESMExports } from 'node:module';

const require = createRequire(import.meta.url);
const entries = ['imported', 'reexported', 'imported-barrel', 'reexported-barrel'];
const namespaces = await Promise.all(
  entries.map((entry) =>
    globalThis.__configName === 'esm'
      ? import(`./dist/${entry}.js`)
      : require(`./dist/${entry}.js`),
  ),
);
const original = https.request;
const replacement = () => {};

try {
  for (const namespace of namespaces) {
    assert.equal(namespace.foo, original);
  }
  https.request = replacement;
  syncBuiltinESMExports();
  for (const [index, namespace] of namespaces.entries()) {
    assert.equal(namespace.foo, replacement, entries[index]);
  }
} finally {
  https.request = original;
  syncBuiltinESMExports();
}
