import { strict as assert } from 'node:assert';

const logs = [];
const originalLog = console.log;
console.log = (...args) => {
  logs.push(args.join(' '));
};

try {
  const main = await import('./dist/main.js');
  await main.load2();

  assert.deepEqual(logs, ['main', 'shared', 'd2']);
} finally {
  console.log = originalLog;
}
