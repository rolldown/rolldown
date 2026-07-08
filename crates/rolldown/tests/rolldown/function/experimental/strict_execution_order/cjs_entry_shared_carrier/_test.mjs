import assert from 'node:assert';

// Entry b's chunk also hosts the shared required-ESM carrier, so entry a's chunk imports it.
// An inline entry trigger would run b's program during a's load; the facade keeps it out.
const logs = [];
const originalLog = console.log;
console.log = (...args) => {
  logs.push(args.join(' '));
};

try {
  await import('./dist/a.js');
  await import('./dist/b.js');
} finally {
  console.log = originalLog;
}

assert.deepStrictEqual(logs, ['S', 'A', 'B']);
