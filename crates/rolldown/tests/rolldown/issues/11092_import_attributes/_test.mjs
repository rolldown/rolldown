import assert from 'node:assert';
import { readFile } from 'node:fs/promises';

// A JSON module has no side effects to observe, so check the import itself. Node then rejects it
// if the `with` clause is missing.
const main = await readFile(new URL('./dist/main.js', import.meta.url), 'utf8');
assert.match(main, /import "data" with \{ type: "json" \};/);
await import('./dist/main.js');
