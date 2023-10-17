const assert = require('node:assert');

module.exports = {
	description: 'removes empty exported var declarations',
	exports(exports) {
		assert.equal(exports.foo, 42);
	}
};
