import assert from 'node:assert';
import { readFileSync } from 'node:fs';

const distUrl = new URL('./dist/main.js', import.meta.url);
const code = readFileSync(distUrl, 'utf8');

assert(!code.includes('init_heavy'), `Expected heavy.js to be unwrapped:\n${code}`);
assert(!code.includes('init_exclusive'), `Expected exclusive.js to be unwrapped:\n${code}`);
assert(code.includes('init_live_lazy'), `Expected live-lazy.js to remain wrapped:\n${code}`);
assert(code.includes('init_shared'), `Expected shared.js to remain wrapped:\n${code}`);
assert(code.includes('require_cjs_dep'), `Expected cjs-dep.cjs to remain CJS-wrapped:\n${code}`);

await import(`${distUrl.href}?t=${Date.now()}`);
const res = await globalThis.__ready;
assert.deepStrictEqual(res, {
  heavy: 'exclusive+shared+cjs',
  live: 'live:shared',
});
