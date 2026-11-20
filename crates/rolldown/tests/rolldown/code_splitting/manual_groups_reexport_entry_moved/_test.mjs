import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';

function stdout(file) {
  return execFileSync(process.execPath, [file], { cwd: import.meta.dirname, encoding: 'utf8' });
}

for (const entry of ['one', 'two']) {
  assert.equal(stdout(`./dist/${entry}.js`), stdout(`./${entry}.js`));
}
