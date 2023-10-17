const assert = require('node:assert');

module.exports = {
	description: 'Assignments should be correctly bound independent of their order',
	exports(exports) {
		assert.equal(exports.baz, 'present');
	}
};
