import assert from 'node:assert';
import { readFile } from 'node:fs/promises';

const code = await readFile(new URL('./dist/main.js', import.meta.url), 'utf8');

assert.match(
  code,
  /export \* from "external" with \{ type: "json" \}/,
  'entry facade must preserve JSON re-export attributes',
);
