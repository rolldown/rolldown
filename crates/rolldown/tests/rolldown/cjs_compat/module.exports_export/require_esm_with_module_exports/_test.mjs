import * as fs from 'node:fs';
import path from 'node:path';
import assert from 'node:assert';

// Read the generated file and verify the optimization works
const file = fs.readFileSync(path.resolve(import.meta.dirname, './dist/main.js'), 'utf-8');

// Since external.mjs exports 'module.exports', the optimization should skip __toCommonJS
// and directly access external_mjs_exports["module.exports"]
assert.ok(file.includes('["module.exports"]') || file.includes("['module.exports']"), 
  'Should directly access module.exports property without __toCommonJS');
assert.ok(!file.includes('__toCommonJS'), 'Should not include __toCommonJS when module.exports export exists');
