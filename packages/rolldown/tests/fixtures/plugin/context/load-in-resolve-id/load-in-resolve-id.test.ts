import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { expect, test } from 'vitest';

function run(scenario: string) {
  const script = fileURLToPath(new URL('./run.mjs', import.meta.url));
  return spawnSync(process.execPath, [script, scenario], {
    encoding: 'utf8',
    timeout: 10_000,
    killSignal: 'SIGKILL',
  });
}

test.each([
  ['this.load in resolveId completes for the input entry', 'input'],
  ['this.load in resolveId completes for the emitted entry', 'emitted'],
  ['this.load in resolveId completes in an import cycle', 'cycle'],
  [
    'this.load returns before the imports are resolved unless resolveDependencies is true',
    'resolve-dependencies',
  ],
  ['the resolveId hook of an entry can wait for another module', 'entry-waits-for-module'],
  ['emitted entries resolve at the same time', 'emitted-entries-wait-for-each-other'],
  [
    'an entry that another entry imports first is an entry in its hooks',
    'entry-imported-by-another-entry',
  ],
])('%s', (_title, scenario) => {
  const { error, status, stderr, stdout } = run(scenario);
  expect(error).toBeUndefined();
  expect(status, stderr).toBe(0);
  expect(stdout.trim()).toBe('42');
});

test.each([
  [
    'this.load in resolveId reports the original entry load error',
    'input-load-error',
    'fixture entry load failed',
  ],
  [
    'an input entry that fails to resolve does not stop other modules from loading',
    'input-entry-error',
    'UNRESOLVED_ENTRY',
  ],
  [
    'an emitted entry that fails to resolve does not stop other modules from loading',
    'emitted-entry-error',
    'UNRESOLVED_ENTRY',
  ],
])('%s', (_title, scenario, message) => {
  const { error, status, stderr } = run(scenario);
  expect(error).toBeUndefined();
  expect(status, stderr).toBe(1);
  expect(stderr).toContain(message);
});
