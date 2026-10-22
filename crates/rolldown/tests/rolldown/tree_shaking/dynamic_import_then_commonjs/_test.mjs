const pending = new Set([
  'eval',
  'module-exports',
  'patched',
  'proto',
  'this-write',
  'tsc-export-const',
  'variable-key',
]);
const allCalled = new Promise((resolve) => {
  globalThis.thenCalled = (name) => {
    pending.delete(name);
    if (pending.size === 0) resolve();
  };
});
await import('./dist/main.js');
await allCalled;
