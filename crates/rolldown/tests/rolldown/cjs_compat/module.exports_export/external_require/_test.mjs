import * as fs from 'node:fs';
import path from 'node:path';
import assert from 'node:assert';

// Read the generated file and verify external modules behavior
const file = fs.readFileSync(path.resolve(import.meta.dirname, './dist/main.js'), 'utf-8');

// External modules should use __require() directly without bundling
// Since ./external.mjs is marked as external, it won't be bundled or optimized
assert.ok(file.includes('__require("./external.mjs")'), 'Should have __require call for external module');
assert.ok(!file.includes('__toCommonJS'), 'Should not include __toCommonJS for external modules');
assert.ok(!file.includes('external_exports'), 'Should not bundle external module');
