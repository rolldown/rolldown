import assert from 'node:assert';

const logs = [];
const originalConsoleLog = console.log;
console.log = (...args) => logs.push(args.join(' '));

try {
  await import('./dist/e0.js');
  assert.deepStrictEqual(
    logs,
    ['sfx-m13-0'],
    'loading the entry must not execute the dynamic dependency',
  );

  await globalThis.__dyn.dyn2();
  assert.deepStrictEqual(
    logs,
    ['sfx-m13-0', 'sfx-m31-0'],
    'the dynamic dependency must execute when its entry is imported',
  );
} finally {
  console.log = originalConsoleLog;
}
