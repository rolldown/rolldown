import assert from 'node:assert';
import fs from 'node:fs';

globalThis.__initLog = [];
const { access } = await import('./dist/main.js');
assert.strictEqual(await access(), 'source,b,a,a');
assert.deepStrictEqual(globalThis.__initLog, ['a.init source', 'a', 'y', 'b']);
// The chunk of the source file keeps its name. The init chunk imports `init_a` from `a.js`.
assert.match(
  fs.readFileSync(new URL('./dist/a.init.js', import.meta.url), 'utf8'),
  /a\.init source/,
);
assert.match(
  fs.readFileSync(new URL('./dist/a2.init.js', import.meta.url), 'utf8'),
  /from "\.\/a\.js"/,
);
delete globalThis.__initLog;
