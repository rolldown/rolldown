import assert from 'node:assert';

// A manual chunk group placing the entry and a deep leaf into the entry chunk displaces the
// leaf behind the common chunk. Source order is [m4, m3, m2, m1, m0].
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

assert.deepStrictEqual(logs, ['m4', 'm3', 'm2', 'm1', 'm0']);
