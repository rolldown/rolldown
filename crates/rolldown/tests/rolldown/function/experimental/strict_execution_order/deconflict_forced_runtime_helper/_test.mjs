import assert from 'node:assert';

// Unfixed: the forced runtime `__esmMin` shares a root binding with helper.js's `__esmMin`; helper's
// init overwrites it with the user string and `init_late`'s `__esmMin(...)` throws. The fix registers
// the runtime statement with the renamer, which renames one so both survive and stay callable.
await import('./dist/main.js');
assert.deepStrictEqual(
  globalThis.__result,
  { helper: 'H:USERVAL', late: 'LATE:z' },
  `strict single-chunk build must keep the runtime helper callable; got ${JSON.stringify(globalThis.__result)}`,
);
