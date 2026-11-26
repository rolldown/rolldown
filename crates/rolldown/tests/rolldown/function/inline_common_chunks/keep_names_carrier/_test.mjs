import assert from 'node:assert/strict';

const names = [];
const log = console.log;
try {
  console.log = (...args) => names.push(args);
  await import('./dist/a.js');
  await import('./dist/b.js');
} finally {
  console.log = log;
}

assert.deepEqual(names, [['Object', 'Object'], ['Object']]);
