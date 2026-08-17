import assert from 'node:assert';
import { readFile } from 'node:fs/promises';

const code = await readFile(new URL('./dist/app.js', import.meta.url), 'utf8');

assert.match(
  code,
  /from "data\.json" with \{ type: "json" \}/,
  'the extracted external star must preserve its import attributes',
);
