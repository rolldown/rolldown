import path from 'node:path';
import { pathToFileURL } from 'node:url';
import assert from 'node:assert';
const distDir = path.join(import.meta.dirname, 'dist');
await import(pathToFileURL(path.join(distDir, 'main.js')).href);
await globalThis.__11397r_done;
assert.deepStrictEqual(Object.keys(globalThis.__11397r_formNs).sort(), ['value']);
