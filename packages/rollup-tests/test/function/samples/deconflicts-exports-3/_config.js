const assert = require('node:assert');

module.exports = {
	description: 'renames variables named "module" if necessary',
	exports(exports) {
		assert.equal(exports, 1);
	}
};
