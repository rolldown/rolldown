import test from 'node:test';

// Run by `node --test`, which hands this child process-wide execArgv entries
// (`--v8-pool-size`, `--node-snapshot`, ...) that a Worker rejects.
test('parallel plugin bootstrap under node --test', async () => {
  await import('./child.mjs');
});
