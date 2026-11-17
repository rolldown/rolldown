import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

for (const [name, expected] of [
  ['entry1', 'unused1\n'],
  ['entry2', 'unused2\n'],
]) {
  const entry = fileURLToPath(new URL(`./dist/${name}.js`, import.meta.url));
  assert.equal(execFileSync(process.execPath, [entry], { encoding: 'utf8' }), expected, name);
}
