import assert from 'node:assert';
import { readdirSync, readFileSync } from 'node:fs';
import { join } from 'node:path';

// `leaf.js` is already loaded for both dynamic entries. It used to stay in a chunk of its own,
// because moving it before `middle.js` closed a cycle with the chunk of `shared.js`.
const distDir = join(import.meta.dirname, 'dist');
const jsFiles = readdirSync(distDir).filter((file) => file.endsWith('.js'));
assert.strictEqual(jsFiles.length, 5, `Expected 5 chunks but got: ${jsFiles.join(', ')}`);
const sharedChunk = jsFiles
  .map((file) => readFileSync(join(distDir, file), 'utf8'))
  .find((code) => code.includes('// shared.js'));
assert(sharedChunk.includes('// middle.js') && sharedChunk.includes('// leaf.js'));

const main = await import('./dist/main1.js');
assert.strictEqual(main.value, 'shared middle leaf');
assert.strictEqual((await main.loadMiddle()).value, 'middle leaf');
assert.strictEqual((await main.loadLeaf()).value, 'leaf');
