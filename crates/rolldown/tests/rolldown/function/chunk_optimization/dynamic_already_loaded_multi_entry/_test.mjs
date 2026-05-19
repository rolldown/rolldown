import assert from 'node:assert';
import { readdirSync, readFileSync } from 'node:fs';
import { join } from 'node:path';

const distDir = join(import.meta.dirname, 'dist');
const jsFiles = readdirSync(distDir).filter((file) => file.endsWith('.js'));

assert.strictEqual(jsFiles.length, 5, `Expected 5 chunks but got: ${jsFiles.join(', ')}`);

const chunks = jsFiles.map((file) => readFileSync(join(distDir, file), 'utf8'));
assert(
  chunks.some((chunk) => chunk.includes('"all"') && chunk.includes('"main1 and main2"')),
  'dependency loaded by both static entries should regroup with the other static-only dependency',
);
assert(
  chunks.some((chunk) => chunk.includes('"main1 and dynamic"')),
  'dependency not already loaded for every dynamic importer should remain separate',
);
