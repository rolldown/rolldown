import assert from 'node:assert';
import { readdirSync, readFileSync } from 'node:fs';
import { join } from 'node:path';

const distDir = join(import.meta.dirname, 'dist');
const jsFiles = readdirSync(distDir).filter((file) => file.endsWith('.js'));

const chunks = jsFiles.map((file) => readFileSync(join(distDir, file), 'utf8'));
const sharedHost = chunks.find((chunk) => chunk.includes('"shared"'));
const dynamicChunk = chunks.find((chunk) => chunk.includes('"dynamic"'));
const disablesAvoidRedundantChunkLoads =
  globalThis.__configName === 'disableAvoidRedundantChunkLoads';

if (disablesAvoidRedundantChunkLoads) {
  assert.strictEqual(jsFiles.length, 3, `Expected 3 chunks but got: ${jsFiles.join(', ')}`);
} else {
  assert.strictEqual(jsFiles.length, 2, `Expected 2 chunks but got: ${jsFiles.join(', ')}`);
}

assert(sharedHost, 'shared module should be emitted');
assert(dynamicChunk, 'dynamic module should be emitted');
if (disablesAvoidRedundantChunkLoads) {
  assert(
    !sharedHost.includes('"main"'),
    'shared module should stay in its own chunk when redundant chunk-load avoidance is disabled',
  );
} else {
  assert(
    sharedHost.includes('"main"'),
    'shared module should be grouped with the statically importing entry',
  );
}
assert(
  !dynamicChunk.includes('"shared"'),
  'dynamic chunk should import the already-loaded shared module instead of duplicating its chunk',
);
