import assert from 'node:assert';

const logs = [];
const originalLog = console.log;
console.log = (...args) => {
  logs.push(args.join(' '));
};

try {
  await import('./dist/main.js');
  await new Promise((resolve) => setImmediate(resolve));
} finally {
  console.log = originalLog;
}

assert.deepStrictEqual(logs, ['read foo', 'read foo']);
