import assert from 'node:assert';
import { readFile } from 'node:fs/promises';

const target = await readFile(new URL('./dist/target.js', import.meta.url), 'utf8');
assert.match(target, /import \* as \w+ from "external" with \{ type: "json" \}/);
