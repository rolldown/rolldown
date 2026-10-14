import { test } from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';

const interrupt = new URL('../lib/interrupt.mjs', import.meta.url).href;

test('Ctrl-C runs every registered cleanup, newest first, then exits 130', () => {
  const script = `
    import { onInterrupt } from ${JSON.stringify(interrupt)};
    onInterrupt(async () => { process.stdout.write('first\\n'); });
    const off = onInterrupt(() => process.stdout.write('removed\\n'));
    off();
    onInterrupt(async () => {
      await new Promise((resolve) => setTimeout(resolve, 50));
      process.stdout.write('second\\n');
    });
    setTimeout(() => {}, 5000);
    process.kill(process.pid, 'SIGINT');
  `;
  const result = spawnSync(process.execPath, ['--input-type=module', '-e', script], {
    encoding: 'utf8',
    timeout: 10_000,
  });
  assert.equal(result.status, 130);
  assert.equal(result.stdout, 'second\nfirst\n');
});
