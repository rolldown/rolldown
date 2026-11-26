import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { readdirSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

function stdout(path) {
  const filename = fileURLToPath(new URL(path, import.meta.url));
  return execFileSync(process.execPath, [filename], { encoding: 'utf8' });
}

for (const entry of ['index', 'index-1']) {
  assert.equal(stdout(`./dist/${entry}.js`), stdout(`./${entry}.ts`));
}

const files = readdirSync(new URL('./dist/', import.meta.url));
assert.ok(
  files.every((file) => !file.startsWith('components')),
  'Expected no components chunk',
);
