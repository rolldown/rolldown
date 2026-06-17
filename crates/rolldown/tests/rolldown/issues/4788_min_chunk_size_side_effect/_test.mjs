import assert from 'node:assert';
import { readdirSync, readFileSync } from 'node:fs';
import { join } from 'node:path';

const distDir = join(import.meta.dirname, 'dist');
const jsFiles = readdirSync(distDir).filter((file) => file.endsWith('.js'));
const chunks = jsFiles.map((file) => readFileSync(join(distDir, file), 'utf8'));

assert.strictEqual(
  jsFiles.length,
  5,
  `Expected side-effect closure to block coalescing: ${jsFiles.join(', ')}`,
);
assert(
  !chunks.some((chunk) => chunk.includes('common1 marker') && chunk.includes('common2 marker')),
  'common chunks with an effectful static dependency closure must not be coalesced',
);
