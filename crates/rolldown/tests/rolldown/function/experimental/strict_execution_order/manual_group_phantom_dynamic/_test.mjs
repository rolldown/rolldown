import assert from 'node:assert';

// m6 is reachable only through a dynamic import that never fires, grouped into an off-cycle
// chunk that main evaluates eagerly. Its own dynamic root is acyclic and self-consistent, so
// only main's phantom seed wraps it — which the cycle bailout must extend, not replace.
// Source order is [Y, X, L, A] and M6 must not appear.
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

assert.deepStrictEqual(logs, ['Y', 'X', 'L', 'A']);
