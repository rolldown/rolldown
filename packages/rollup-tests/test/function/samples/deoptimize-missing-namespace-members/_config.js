const assert = require('node:assert');

module.exports = {
	description: 'handles deoptimization of non-existing namespace members',
	exports(exports) {
		assert.strictEqual(exports.bar, undefined);
	}
};
