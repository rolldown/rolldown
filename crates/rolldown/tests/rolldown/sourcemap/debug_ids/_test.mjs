const require = (await import('node:module')).createRequire(import.meta.url);
const fs = require('node:fs');
const assert = require('node:assert');
const path = require('node:path');

const source = fs.readFileSync(path.resolve(__dirname, 'dist/assets/main.js'), 'utf8');
const match = source.match(/\/\/# debugId=([a-fA-F0-9-]+)/);

assert.ok(match, 'Could not find debugId in source');
const sourceDebugId = match[1];

const sourceMap = JSON.parse(fs.readFileSync(path.resolve(__dirname, 'dist/assets/main.js.map'), 'utf8'));
assert.equal(sourceMap.debugId, sourceDebugId, 'debugId mismatch');
