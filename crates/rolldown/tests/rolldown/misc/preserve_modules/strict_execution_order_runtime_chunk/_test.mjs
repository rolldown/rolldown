import assert from 'node:assert';

globalThis.__events = [];
await import('./dist/main.js');

// `preserveModules` emits one file per module plus the shared `rolldown-runtime` chunk that strict
// execution order splits out. That runtime chunk mirrors no user module, which used to make the
// `preserveModules` export naming assume every chunk has an entry module and panic.
assert.deepStrictEqual(globalThis.__events, ['dep', 'main DN', 'lazy', 'after L']);
