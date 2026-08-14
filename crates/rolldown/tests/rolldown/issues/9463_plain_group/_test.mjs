import assert from 'node:assert';

// https://github.com/rolldown/rolldown/issues/9463 (plain-group variant).
// Loading entry `b` must run only b's own side effects, never entry `a`'s.
globalThis.sideEffectLog = [];
await import(new URL('./dist/b.js', import.meta.url));
await new Promise((resolve) => setTimeout(resolve, 0));

assert.ok(
  !globalThis.sideEffectLog.includes('a'),
  `executing entry "b" must not trigger entry "a"'s side effect; got ${JSON.stringify(globalThis.sideEffectLog)}`,
);
assert.deepStrictEqual(globalThis.sideEffectLog, ['b', 'shared-foo']);
