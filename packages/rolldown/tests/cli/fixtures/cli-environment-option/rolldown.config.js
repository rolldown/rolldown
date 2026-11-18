import assert from 'node:assert/strict';
import { readdirSync, readFileSync } from 'node:fs';
import { setTimeout } from 'node:timers/promises';
import { defineConfig } from 'rolldown';

export default defineConfig(async () => {
  // Check that environment variables are set correctly
  assert.strictEqual(process.env.PRODUCTION, 'true');
  assert.strictEqual(process.env.FOO, 'bar');
  assert.strictEqual(process.env.HOST, 'http://localhost:4000');
  if (process.env.ASSERT_ROLLDOWN_WORKER_THREADS) {
    // Let newly created threads finish setting their names before inspecting /proc.
    await setTimeout(100);
    const workers = readdirSync('/proc/self/task').filter((tid) => {
      try {
        return readFileSync(`/proc/self/task/${tid}/comm`, 'utf8').trim() === 'rolldown-worker';
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
