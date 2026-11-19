import assert from 'node:assert';
import { access, readFile } from 'node:fs/promises';
import { pages } from './dist/entry.js';

const result = await pages[0].load();
assert.strictEqual(result.message, 'Hello from page.js');

const unused = new URL('./dist/unused.js', import.meta.url);
if (globalThis.__configName === 'treeshake-disabled') {
  await access(unused);
  const code = await readFile(new URL('./dist/entry.js', import.meta.url), 'utf8');
  assert.match(code, /import\("\.\/unused\.js"\)/);
} else {
  await assert.rejects(access(unused), { code: 'ENOENT' });
}
