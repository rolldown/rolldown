const assert = require('node:assert');

module.exports = {
	description: 'export from does not create a local binding',
	runtimeError(error) {
		assert.ok(/foo is not defined/.test(error.message));
	}
};
