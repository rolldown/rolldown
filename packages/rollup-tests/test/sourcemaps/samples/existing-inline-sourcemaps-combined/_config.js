const assert = require('node:assert');

module.exports = {
	description: 'removes existing inline sourcemaps',
	async test(code) {
		assert.ok(!code.includes('sourceMappingURL=data'));
	}
};
