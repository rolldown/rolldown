const assert = require('node:assert');

module.exports = {
	description: 'allows external module to be configured (b)',
	options: {
		external: ['path']
	},
	exports() {
		assert.equal(require('node:path').resolve.configured, 'yes');
	}
};
