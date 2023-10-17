const assert = require('node:assert');

module.exports = {
	description: 'detects side-effects in nested .forEach calls (#3533)',
	exports: exports => assert.strictEqual(exports.result, 9)
};
