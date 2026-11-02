import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';

// A fresh process for each entry, so one entry cannot load the style for another.
for (const name of ['through_barrel', 'direct', 'namespace']) {
  const entry = JSON.stringify(new URL(`./dist/${name}.js`, import.meta.url).href);
  const script = `await import(${entry}); console.log(JSON.stringify(globalThis.styleLoaded ?? null));`;
  const styleLoaded = JSON.parse(
    execFileSync(process.execPath, ['--input-type=module', '-e', script], { encoding: 'utf8' }),
  );
  assert.equal(styleLoaded, true, name);
}
