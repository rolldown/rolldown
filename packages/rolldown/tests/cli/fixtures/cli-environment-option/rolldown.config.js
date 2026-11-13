import assert from 'node:assert/strict';
import { readdirSync, readFileSync } from 'node:fs';
import { setTimeout } from 'node:timers/promises';
import { defineConfig, rolldown } from 'rolldown';
import { getRuntimeSupport } from 'rolldown/experimental';

export default defineConfig(async () => {
  // Check that environment variables are set correctly
  assert.strictEqual(process.env.PRODUCTION, 'true');
  assert.strictEqual(process.env.FOO, 'bar');
  assert.strictEqual(process.env.HOST, 'http://localhost:4000');
  // The binding reads `ROLLDOWN_RUNTIME` once at load, so `--environment` must
  // apply first. Only threaded WASI lets it select CurrentThread (no `dev`);
  // native ignores it and stays MultiThread.
  const support = getRuntimeSupport();
  if (support.parallelPlugins) {
    assert.strictEqual(support.dev, true);
  } else if (!support.threadlessWasi) {
    assert.strictEqual(support.dev, false);
  }
  if (process.env.ASSERT_ROLLDOWN_WORKER_THREADS) {
    // The worker pool starts on the first async binding call.
    const bundle = await rolldown({
      input: 'entry',
      plugins: [
        {
          name: 'virtual',
          resolveId: (id) => (id === 'entry' ? id : null),
          load: (id) => (id === 'entry' ? 'export default 1' : null),
        },
      ],
    });
    await bundle.generate();
    await bundle.close();
    // Let newly created threads finish setting their names before inspecting /proc.
    await setTimeout(100);
    const workers = readdirSync('/proc/self/task').filter((tid) => {
      try {
        // Linux keeps 15 bytes of a thread name: 'rolldown-runtime-N' reads as 'rolldown-runtim'.
        return readFileSync(`/proc/self/task/${tid}/comm`, 'utf8').trim() === 'rolldown-runtim';
      } catch (error) {
        // Other threads can exit between listing tasks and reading their names.
        if (error.code === 'ENOENT') return false;
        throw error;
      }
    });
    assert.strictEqual(workers.length, 2);
  }
  return {
    input: './index.js',
  };
});
