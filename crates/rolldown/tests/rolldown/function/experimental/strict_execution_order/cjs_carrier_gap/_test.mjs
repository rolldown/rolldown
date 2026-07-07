import assert from 'node:assert';

// Under strictExecutionOrder, a code-split CJS side effect (`c`) reached through a bare-import
// pass-through (`p`) must not run before an eager ESM sensitive module (`e`). Source order is
// [E, C, MAIN]; without the carrier-sensitivity fix strict wrongly emits [C, E, MAIN].
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

assert.deepStrictEqual(logs, ['E', 'C', 'MAIN']);
