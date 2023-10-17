const assert = require('node:assert');

module.exports = {
	description:
		'generates output file when multiple configurations are specified and one build fails',
	command: 'rollup -c',
	error: error => {
		assert.ok(/Unexpected Exception/.test(error.message));
		return true;
	}
};
