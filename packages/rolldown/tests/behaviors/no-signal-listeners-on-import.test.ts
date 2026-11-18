import { execFileSync } from 'node:child_process';

import { expect, test } from 'vitest';

// Importing rolldown must not install signal handlers (via `signal-exit`) unless a trace
// subscriber is active. Those handlers re-raise the signal, which keeps a process with an
// attached debugger (`node --inspect`) alive after SIGHUP/SIGINT/SIGTERM.
test('importing rolldown does not add signal listeners when tracing is disabled', () => {
  const env = { ...process.env };
  delete env.RD_LOG;
  delete env.RD_LOG_OUTPUT;

  const output = execFileSync(
    process.execPath,
    [
      '--input-type=module',
      '-e',
      `await import('rolldown');
       console.log(JSON.stringify(['SIGHUP', 'SIGINT', 'SIGTERM'].map((s) => process.listenerCount(s))));`,
    ],
    { cwd: import.meta.dirname, env, encoding: 'utf8' },
  );

  expect(JSON.parse(output.trim())).toEqual([0, 0, 0]);
});
