import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

for (const name of ['entry1', 'entry2', 'entry3']) {
  const source = fileURLToPath(new URL(`./${name}.js`, import.meta.url));
  const output = fileURLToPath(new URL(`./dist/${name}.js`, import.meta.url));
  assert.equal(
    execFileSync(process.execPath, [output], { encoding: 'utf8' }),
    execFileSync(process.execPath, [source], { encoding: 'utf8' }),
    name,
  );
}
