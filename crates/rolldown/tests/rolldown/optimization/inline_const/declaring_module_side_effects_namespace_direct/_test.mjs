import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';

// A fresh process for each entry, so one entry cannot run `button.js` for the other.
for (const name of ['namespace', 'named']) {
  const entry = JSON.stringify(new URL(`./dist/${name}.js`, import.meta.url).href);
  const script = `await import(${entry}); console.log(JSON.stringify([globalThis.styleLoaded ?? null, globalThis.size ?? null]));`;
  const result = JSON.parse(
    execFileSync(process.execPath, ['--input-type=module', '-e', script], { encoding: 'utf8' }),
  );
  assert.deepEqual(result, [true, 'md'], name);
}
