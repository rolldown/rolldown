import assert from 'node:assert';
import fs from 'node:fs';

const { loaded } = await import('./dist/main.js');
assert.strictEqual(await loaded, 7);

const main = fs.readFileSync(new URL('./dist/main.js', import.meta.url), 'utf8');
// `with:` can only come from an import attributes object.
assert.strictEqual(main.includes('with:'), false, `the attribute must not survive:\n${main}`);
assert.strictEqual(
  main.includes('"./data.json"'),
  false,
  `the source specifier must be rewritten:\n${main}`,
);
