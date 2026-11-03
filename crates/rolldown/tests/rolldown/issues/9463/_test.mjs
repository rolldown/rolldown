import assert from 'node:assert';

// Behavioral regression test for https://github.com/rolldown/rolldown/issues/9463.
//
// Each module records its top-level side effect in `globalThis.sideEffectLog`.
// Executing entry `b` (which reaches `shared.js` only via a dynamic import) must
// run ONLY b's own side effects — never entry `a`'s. Before the fix, entry `b`'s
// chunk imported entry `a`'s chunk and eagerly ran `init_a()`, so loading `b`
// pushed `'a'`.

globalThis.sideEffectLog = [];

// Load ONLY entry `b`.
await import(new URL('./dist/b.js', import.meta.url));
// Let the dynamic `import('./shared.js')` continuation settle.
await new Promise((resolve) => setTimeout(resolve, 0));

assert.ok(
  !globalThis.sideEffectLog.includes('a'),
  `executing entry "b" must not trigger entry "a"'s side effect; got ${JSON.stringify(globalThis.sideEffectLog)}`,
);
assert.deepStrictEqual(
  globalThis.sideEffectLog,
  ['b', 'shared-foo'],
  `entry "b" should run only its own side effect plus its dynamic import; got ${JSON.stringify(globalThis.sideEffectLog)}`,
);

// Sanity: entry `a`'s side effect is not dead — it runs when `a` itself is loaded.
await import(new URL('./dist/a.js', import.meta.url));
assert.ok(
  globalThis.sideEffectLog.includes('a'),
  `entry "a"'s side effect should run when entry "a" is executed; got ${JSON.stringify(globalThis.sideEffectLog)}`,
);
