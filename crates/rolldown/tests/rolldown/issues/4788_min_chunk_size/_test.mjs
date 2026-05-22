import assert from 'node:assert';
import { readdirSync, readFileSync } from 'node:fs';
import { join } from 'node:path';

const distDir = join(import.meta.dirname, 'dist');
const jsFiles = readdirSync(distDir).filter((file) => file.endsWith('.js'));
const chunks = jsFiles.map((file) => readFileSync(join(distDir, file), 'utf8'));
const mergedCommonChunkCount = chunks.filter(
  (chunk) => chunk.includes('common1 marker') && chunk.includes('common2 marker'),
).length;

if (globalThis.__configName?.startsWith('disabled-')) {
  assert.strictEqual(
    jsFiles.length,
    5,
    `Expected minChunkSize to be disabled: ${jsFiles.join(', ')}`,
  );
  assert.strictEqual(
    mergedCommonChunkCount,
    0,
    'common chunks should not be coalesced when disabled',
  );
} else {
  assert.strictEqual(
    jsFiles.length,
    4,
    `Expected 3 entries + 1 common chunk: ${jsFiles.join(', ')}`,
  );
  assert.strictEqual(
    mergedCommonChunkCount,
    1,
    'common1/common2 should be coalesced into one chunk',
  );
}
