import { execa } from 'execa';
import nodePath from 'node:path';
import { describe, expect, test } from 'vitest';

// Smoke test for the in-process dev-engine lifecycle the browser harness
// builds on (see meta/design/dev-server-test-harness.md, "Server entry point").
// Nothing else exercises `DevEngine.close()`: the node fixtures tear their
// subprocess servers down with killPort/SIGKILL instead.
describe('dev engine close path', () => {
  // Run the create → build → close cycle twice in a bare `node` child and
  // assert it exits on its own. A vitest worker cannot assert this on itself:
  // the pool force-terminates workers, which would mask a leaked watcher
  // thread or napi threadsafe callback keeping the event loop alive.
  test('close() releases resources so the process can exit', async () => {
    const child = await execa(
      process.execPath,
      [nodePath.resolve(__dirname, 'src/dev-engine-close-child.mjs')],
      { timeout: 60_000 },
    );
    expect(child.exitCode).toBe(0);
    expect(child.stdout).toContain('dev-engine-close-child: OK');
  });

  // The same cycle in-process — the environment the new harness actually runs
  // engines in (a vitest fork).
  test('a second engine can start after the first closes', async () => {
    const { dev } = await import('rolldown/experimental');
    const fixtureDir = nodePath.resolve(__dirname, 'fixtures/edit');
    for (let round = 0; round < 2; round++) {
      const engine = await dev(
        { cwd: fixtureDir, input: ['./src/main.js'], treeshake: false },
        {},
        { watch: { skipWrite: true } },
      );
      await engine.run();
      await engine.close();
    }
  });
});
