import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';

// `owner.js` reaches `impl.js` only through `barrel.js`, which has no side effects, so tree
// shaking does not keep `owner.js` for the read of `ui.SIZE` in `a`. Every mode must agree.
// A fresh process for each entry, so one entry cannot run `impl.js` for another.
const implLoaded = {};
for (const name of ['a', 'b']) {
  const entry = JSON.stringify(new URL(`./dist/${name}.js`, import.meta.url).href);
  const script = `await import(${entry}); console.log(JSON.stringify(globalThis.implLoaded ?? null));`;
  implLoaded[name] = JSON.parse(
    execFileSync(process.execPath, ['--input-type=module', '-e', script], { encoding: 'utf8' }),
  );
}
assert.equal(implLoaded.a, null);
assert.equal(implLoaded.b, true);
