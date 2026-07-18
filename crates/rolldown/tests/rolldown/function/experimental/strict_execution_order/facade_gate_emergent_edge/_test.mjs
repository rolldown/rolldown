import assert from 'node:assert';

globalThis.__events = [];

// Load only the importing side (the root) and confirm `sec`'s program did not run. This asserts the
// fixed output executes correctly with `sec` deferred behind its facade; it does not by itself
// distinguish the bug, since the emergent A->gs edge is a conservative projection the real lowering
// never renders (chunk `a.js` imports nothing from `sec`'s chunk), so this also holds on the unfixed
// build. The regression guard is the SNAPSHOT: without the fix `sec.js` keeps its inline
// `require_sec()` trigger and no `sec2.js` facade split; with the fix the gate sees the emergent
// edge and splits the facade.
await import('./dist/main.js');
assert.ok(
  !globalThis.__secRan,
  `sec must not run when only the root is loaded; ran ${globalThis.__secRan} time(s)`,
);

// Loading `sec` directly runs its CommonJS program exactly once and yields its exports.
const sec = await import('./dist/sec.js');
assert.strictEqual(globalThis.__secRan, 1, 'sec runs once when loaded directly');
assert.deepStrictEqual(sec.default, { s: 'TV' }, 'sec exports its CommonJS value');
