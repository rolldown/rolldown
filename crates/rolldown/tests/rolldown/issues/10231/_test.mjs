import { createRequire } from 'node:module';
import assert from 'node:assert';

const require = createRequire(import.meta.url);
const rule = require('./dist/rule.js');
const plugin = require('./dist/index.js');

assert.strictEqual(rule.name, 'test', 'rule.js should assign the default export to module.exports');
assert.strictEqual(plugin.rules['test-rule'], rule);
