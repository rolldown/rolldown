const assert = require('node:assert');

module.exports = {
	description: "namespace's 'default' properties should be available",

	exports(exports) {
		assert.equal(exports, 42);
	}
};
