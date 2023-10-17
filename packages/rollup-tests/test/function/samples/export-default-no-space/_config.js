const assert = require('node:assert');

module.exports = {
	description: 'handles default exports with no space before declaration',
	exports: exports => {
		assert.deepEqual(exports, {});
	}
};
