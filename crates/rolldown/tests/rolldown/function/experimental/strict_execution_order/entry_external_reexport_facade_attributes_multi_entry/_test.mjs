import assert from 'node:assert';
import { readFile } from 'node:fs/promises';

const a = await readFile(new URL('./dist/a.js', import.meta.url), 'utf8');
const b = await readFile(new URL('./dist/b.js', import.meta.url), 'utf8');

assert.match(
  a,
  /export \* from "external" with \{ type: "json" \}/,
  'entry a must preserve JSON re-export attributes',
);
assert.match(
  b,
  /export \* from "external" with \{ type: "css" \}/,
  'entry b must preserve CSS re-export attributes',
);
