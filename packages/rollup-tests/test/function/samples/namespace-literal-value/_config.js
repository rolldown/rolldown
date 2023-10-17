const assert = require('node:assert');

module.exports = {
	description: 'does not simplify accessing unknown properties from namespaces',
	exports({ isNull }) {
		assert.strictEqual(isNull('a'), true);
		assert.strictEqual(isNull('b'), false);
	}
};
