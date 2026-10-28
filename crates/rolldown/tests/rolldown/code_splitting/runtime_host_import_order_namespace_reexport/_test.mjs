import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import path from 'node:path';

// Keep source and output in separate processes. Both must initialize the global before the read.
for (const file of ['c.js', 'dist/c.js']) {
  const entry = JSON.stringify(path.join(import.meta.dirname, file));
  const script = `const { c } = await import(${entry}); console.log(JSON.stringify(c.value));`;
  const value = JSON.parse(
    execFileSync(process.execPath, ['--input-type=module', '-e', script], { encoding: 'utf8' }),
  );
  assert.equal(value, 'ready', `${file} must keep the initializer before the snapshot`);
}
