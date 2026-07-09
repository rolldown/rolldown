import assert from 'node:assert/strict';
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';

import { installCurrentThreadTaskHost } from '../install-current-thread-task-host.mjs';

delete process.env.RD_LOG;
delete process.env.RD_LOG_OUTPUT;
const binding = await import('../../../src/binding.cjs');
const { BindingBundler, BindingLogLevel, initTraceSubscriber } = binding;
const uninstallCurrentThreadTaskHost = installCurrentThreadTaskHost(binding);

const root = mkdtempSync(path.join(tmpdir(), 'rolldown-devtools-first-'));
writeFileSync(path.join(root, 'main.js'), 'export const value = 1;\n');

try {
  const traced = new BindingBundler();
  const result = await traced.generate({
    inputOptions: {
      cwd: root,
      devtools: { sessionId: 'devtools-first' },
      input: [{ import: './main.js' }],
      logLevel: BindingLogLevel.Silent,
      onLog() {},
      plugins: [],
    },
    outputOptions: { plugins: [] },
  });
  assert.equal(result?.isBindingErrors, undefined);
  await traced.close();

  process.env.RD_LOG = 'info';
  process.env.RD_LOG_OUTPUT = 'readable';
  assert.equal(initTraceSubscriber(), null);

  console.log(JSON.stringify({ devtoolsFirst: true, rdLogRejected: true }));
} finally {
  uninstallCurrentThreadTaskHost();
  rmSync(root, { force: true, recursive: true });
}
