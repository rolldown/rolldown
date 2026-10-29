import assert from 'node:assert';

globalThis.__entryInteropOrder = [];
await import('./dist/e1.js');
assert.deepStrictEqual(
  globalThis.__entryInteropOrder,
  ['m5', 'm2', 'e1'],
  'internal wrapped entry must preserve source execution order',
);
delete globalThis.__entryInteropOrder;
