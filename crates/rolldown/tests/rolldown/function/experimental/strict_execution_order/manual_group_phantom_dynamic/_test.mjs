import assert from 'node:assert';

// m6 is only reachable through a dynamic import that never fires. The manual group places it
// next to eagerly needed modules inside a chunk cycle; the cycle bailout must still wrap the
// phantom so it never runs. Source order is [M4, B, A] and M6 must not appear.
const logs = [];
const originalLog = console.log;
console.log = (...args) => {
  logs.push(args.join(' '));
};

try {
  await import('./dist/main.js');
} finally {
  console.log = originalLog;
}

assert.deepStrictEqual(logs, ['M4', 'B', 'A']);
