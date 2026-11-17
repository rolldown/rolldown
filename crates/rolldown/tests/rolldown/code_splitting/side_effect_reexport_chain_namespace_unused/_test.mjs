import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

for (const [name, expected] of [
  ['entry1', 'entry1 bar\n'],
  ['entry2', 'entry2 bar\n'],
]) {
  const entry = fileURLToPath(new URL(`./dist/${name}.js`, import.meta.url));
  assert.equal(execFileSync(process.execPath, [entry], { encoding: 'utf8' }), expected, name);
}
