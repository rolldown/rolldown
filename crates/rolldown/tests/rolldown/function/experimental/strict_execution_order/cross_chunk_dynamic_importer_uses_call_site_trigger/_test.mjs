import assert from 'node:assert';
import { readFile } from 'node:fs/promises';

// Neither dynamic importer needs a facade chunk (asserted by the snapshot): each
// import() rewrite carries the trigger itself. Executing entry `b` must still initialize
// `target` without triggering entry `a`'s side effect, and `target` must still initialize
// exactly once across both entries — the guarantee survives losing the file.

globalThis.log = [];

const { bTargetPromise } = await import(new URL('./dist/b.js', import.meta.url));
const nsFromB = await bTargetPromise;
assert.strictEqual(nsFromB.value, 1);
assert.deepStrictEqual(globalThis.log, ['b', 'target']);

const { aTargetPromise } = await import(new URL('./dist/a.js', import.meta.url));
const nsFromA = await aTargetPromise;
assert.strictEqual(nsFromA.value, 1);
assert.deepStrictEqual(globalThis.log, ['b', 'target', 'a']);

// Reading the other target exports from either source-level dynamic import would widen
// DynamicImportExportsUsage and stop testing the simulated facade's narrowed interface.
// Inspect the namespace object published by the implementation chunk instead.
const aOutputUrl = new URL('./dist/a.js', import.meta.url);
const aOutput = await readFile(aOutputUrl, 'utf8');
const implementationSpecifiers = [...aOutput.matchAll(/\bfrom\s+["']([^"']+)["']/g)].map(
  (match) => match[1],
);
const implementationModules = await Promise.all(
  implementationSpecifiers.map((specifier) => import(new URL(specifier, aOutputUrl))),
);
const targetNamespace = implementationModules
  .flatMap((implementation) => Object.values(implementation))
  .find((value) => value && typeof value === 'object' && 'value' in value);
assert.ok(targetNamespace, 'the collapsed dynamic entry should publish its simulated namespace');
assert.doesNotThrow(
  () => ({ ...targetNamespace }),
  'every simulated namespace getter should resolve to a live binding',
);
assert.deepStrictEqual(
  Object.keys(targetNamespace),
  ['value'],
  'retained aliases must not widen the simulated namespace',
);
