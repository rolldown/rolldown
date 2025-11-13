import * as fs from 'node:fs';
import path from 'node:path';
import assert from 'node:assert';

// Read the generated file and verify it doesn't call __toCommonJS for external modules
const file = fs.readFileSync(path.resolve(import.meta.dirname, './dist/main.js'), 'utf-8');

// External modules should use __require() directly without __toCommonJS
assert.ok(file.includes('__require("external")'), 'Should have __require call for external module');
assert.ok(!file.includes('__toCommonJS'), 'Should not include __toCommonJS for external modules');
