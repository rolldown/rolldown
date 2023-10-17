const assert = require('node:assert');

module.exports = {
	description: 'Associates function parameters with their call arguments with regard to mutations',
	exports(exports) {
		assert.equal(exports.bar, 'present');
	}
};
