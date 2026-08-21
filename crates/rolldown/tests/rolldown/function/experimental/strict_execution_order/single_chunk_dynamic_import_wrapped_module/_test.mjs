import assert from 'node:assert';

globalThis.log = [];

await import('./dist/main.js');
await globalThis.ready;

assert.deepStrictEqual(globalThis.log, [
  // Eager phase: the shared dependency keeps its static position, and nothing the dynamic
  // target pulls in has run by the time the entry body finishes.
  'shared',
  'main-body',
  'lazy-pending:true',
  // Deferred phase: one evaluation of the wrapped target and its own dependency, in source
  // order, with both `import()` call sites settling to the same namespace object.
  'lazy-dep',
  'lazy-body',
  'lazy-resolved:dep:shared',
  'same-namespace:true',
]);
