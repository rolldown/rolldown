import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

for (const [name, expected] of [
  ['entry1', 'side effect\nentry1 foo\n'],
  ['entry2', 'side effect\nentry2 foo\n'],
  ['entry3', 'entry3 foo\n'],
]) {
  const entry = fileURLToPath(new URL(`./dist/${name}.js`, import.meta.url));
  assert.equal(execFileSync(process.execPath, [entry], { encoding: 'utf8' }), expected, name);
}
