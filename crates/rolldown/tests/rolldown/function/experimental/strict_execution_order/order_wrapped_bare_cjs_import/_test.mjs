import assert from 'node:assert';

// m1's bare import of c3 must keep its require trigger inside the order wrapper even though m2
// also reaches c3 through a value import. Source order is [c3, c4, m2, m1, m0].
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

assert.deepStrictEqual(logs, ['c3', 'c4', 'm2', 'm1', 'm0']);
