import assert from 'node:assert';

// The manual group splits the CJS wrapper definition (m3, in the group chunk) from its eager
// carrier trigger (m1, in the common chunk). The chunk cycle used to call the not-yet-assigned
// `var require_m3` and crash; source order is [m3, m1, m2, m0].
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

assert.deepStrictEqual(logs, ['m3', 'm1', 'm2', 'm0']);
