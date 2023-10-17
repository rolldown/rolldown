const assert = require('node:assert');

module.exports = {
	description: 'ignores conflict between local definitions and export from declaration',
	exports(exports) {
		assert.equal(exports.foo, 'a-bar');
		assert.equal(exports.bar, 'a-foo');
		assert.equal(exports.baz, 'a-baz');
	}
};

// https://github.com/rollup/rollup/issues/16
