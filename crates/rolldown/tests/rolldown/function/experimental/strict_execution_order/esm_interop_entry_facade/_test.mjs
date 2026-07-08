import assert from 'node:assert';

// e is an ESM user entry that is also require()d, so it carries an interop wrapper. Entry a's
// chunk imports e's chunk for the binding; the trigger must live in e's facade, not inline.
const logs = [];
const originalLog = console.log;
console.log = (...args) => {
  logs.push(args.join(' '));
};

try {
  await import('./dist/a.js');
  await import('./dist/b.js');
  await import('./dist/e.js');
} finally {
  console.log = originalLog;
}

assert.deepStrictEqual(logs, ['E', 'A', 'B']);
