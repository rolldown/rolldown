import assert from 'node:assert/strict';
import { defineConfig } from 'rolldown';
import { getAsyncRuntimeConfig } from 'rolldown/experimental';

export default defineConfig(() => {
  // Check that environment variables are set correctly
  assert.strictEqual(process.env.PRODUCTION, 'true');
  assert.strictEqual(process.env.FOO, 'bar');
  assert.strictEqual(process.env.HOST, 'http://localhost:4000');
  const runtime = getAsyncRuntimeConfig();
  assert.strictEqual(runtime.workerThreads, runtime.flavor === 'CurrentThread' ? 1 : 3);
  return {
    input: './index.js',
  };
});
