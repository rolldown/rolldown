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

test.each(['input', 'emitted'])('this.load in resolveId completes for the %s entry', (scenario) => {
  const { error, status, stderr, stdout } = run(scenario);
  expect(error).toBeUndefined();
  expect(status, stderr).toBe(0);
  expect(stdout.trim()).toBe('42');
});

test('this.load in resolveId reports the original entry load error', () => {
  const { error, status, stderr } = run('input-load-error');
  expect(error).toBeUndefined();
  expect(status, stderr).toBe(1);
  expect(stderr).toContain('fixture entry load failed');
});
