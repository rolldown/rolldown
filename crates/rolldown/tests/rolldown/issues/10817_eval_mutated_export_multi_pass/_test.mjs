import assert from 'node:assert/strict';

const logs = [];
const { log } = console;
console.log = (...args) => logs.push(args.join(' '));
try {
  await import('./dist/main.js');
} finally {
  console.log = log;
}
assert.deepEqual(logs, ['5 bbbbb']);
