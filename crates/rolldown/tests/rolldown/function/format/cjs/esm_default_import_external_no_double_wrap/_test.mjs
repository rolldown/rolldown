import assert from 'node:assert';
import fs from 'node:fs';
import { createRequire } from 'node:module';
import path from 'node:path';

const dir = import.meta.dirname;
const bundlePath = path.resolve(dir, 'dist/main.js');
const code = fs.readFileSync(bundlePath, 'utf-8');

// The importer is ESM ("type": "module"), so the emitted interop must stay
// in node mode — the fix lives in the runtime helper, not in emission.
assert.ok(code.includes('__toESM(esm_pkg, 1)'), 'should use node-mode __toESM for ESM importer');

// Executing the CJS bundle must resolve the external's real default export,
// matching what the source got under real Node ESM. Copy to a `.cjs`
// extension so the surrounding `"type": "module"` package.json does not make
// `require()` treat the bundle itself as ESM.
const cjsPath = path.resolve(dir, 'dist/main.cjs');
fs.copyFileSync(bundlePath, cjsPath);
const require = createRequire(import.meta.url);
const { result } = require(cjsPath);
assert.strictEqual(result, 'Hello, rolldown!');
