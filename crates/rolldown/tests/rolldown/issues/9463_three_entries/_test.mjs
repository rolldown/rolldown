import assert from 'node:assert';

// https://github.com/rolldown/rolldown/issues/9463 (three-entry variant).
// Loading entry `b` must run only b's own side effects — not entry `a`'s or `c`'s.
globalThis.sideEffectLog = [];
await import(new URL('./dist/b.js', import.meta.url));
await new Promise((resolve) => setTimeout(resolve, 0));

assert.ok(
  !globalThis.sideEffectLog.includes('a') && !globalThis.sideEffectLog.includes('c'),
  `executing entry "b" must not trigger other entries' side effects; got ${JSON.stringify(globalThis.sideEffectLog)}`,
);
assert.deepStrictEqual(globalThis.sideEffectLog, ['b', 'shared-foo']);
