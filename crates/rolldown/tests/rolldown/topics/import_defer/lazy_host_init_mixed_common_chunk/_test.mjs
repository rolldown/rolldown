import assert from 'node:assert';
import { execFileSync } from 'node:child_process';

globalThis.__initLog = [];
const { access } = await import('./dist/main.js');
// `w.js` and `y.js` read the log when they run.
assert.deepStrictEqual(await access(), [['a,w', 'a,w,a2,y'], 'aa2']);
assert.deepStrictEqual(globalThis.__initLog, ['a', 'w', 'a2', 'y']);
delete globalThis.__initLog;

// A fresh process loads the chunk of `r.js` first. Its load runs `w.js`, and the deferred modules
// run only at the read of their namespaces.
const r = JSON.stringify(new URL('./dist/r.js', import.meta.url).href);
const script = `globalThis.__initLog = []; const { read } = await import(${r}); const atLoad = [...globalThis.__initLog]; read(); console.log(JSON.stringify([atLoad, globalThis.__initLog]));`;
const logs = JSON.parse(
  execFileSync(process.execPath, ['--input-type=module', '-e', script], { encoding: 'utf8' }),
);
assert.deepStrictEqual(logs, [['w'], ['w', 'a', 'a2']]);
