import { strict as assert } from 'node:assert';
import { readFile } from 'node:fs/promises';

const entry = new URL(`./dist/${globalThis.__configName}/entries/main.js`, import.meta.url);
const code = await readFile(entry, 'utf8');
assert.ok(!code.includes('__ROLLDOWN_ASSET__#'));
assert.ok(!code.includes('__ROLLDOWN_COPY_MODULE__#'));
const result = await import(entry);

if (globalThis.__configName.startsWith('asset_')) {
  assert.equal(result.path, '../assets/data.txt');
  assert.equal(result.length, result.path.length);
  assert.equal(result.startsWithDot, true);
  assert.equal(result.suffixed, '../assets/data.txt?download');
  assert.equal(await readFile(new URL(result.path, entry), 'utf8'), 'asset contents\n');
} else {
  assert.equal(result.copied, 'copied module executed');
  assert.equal(result.reexported, result.copied);
  assert.equal((await result.load()).default, result.copied);
  assert.equal(globalThis.copyExecutions, 1);
}
