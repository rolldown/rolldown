import assert from 'node:assert/strict';

globalThis.evaluations = 0;
const failures = [];
for (const entry of ['./dist/a.js', './dist/b.js']) {
  try {
    await import(entry);
  } catch (error) {
    failures.push(error);
  }
}

assert.equal(globalThis.evaluations, 1);
assert.deepEqual(failures, [undefined, undefined]);
