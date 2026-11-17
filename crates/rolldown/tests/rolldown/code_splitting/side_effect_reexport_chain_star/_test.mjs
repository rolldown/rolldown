import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

for (const [name, expected] of [
  ['entry1', 'entry1 foo\n'],
  ['entry2', 'entry2 foo\n'],
  ['entry3', 'entry3 foo\n'],
]) {
  const entry = fileURLToPath(new URL(`./dist/${name}.js`, import.meta.url));
  assert.equal(execFileSync(process.execPath, [entry], { encoding: 'utf8' }), expected, name);
}
