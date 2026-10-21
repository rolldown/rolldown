import assert from 'node:assert/strict';
import { defineConfig } from 'rolldown';
import { getRuntimeSupport } from 'rolldown/experimental';

export default defineConfig(() => {
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
  return {
    input: './index.js',
  };
});
