const assert = require('node:assert');

module.exports = {
	description: 'uses correct "this" in dynamic imports when not using arrow functions',
	context: {
		require(id) {
			switch (id) {
				case 'input': {
					return { outputPath: 'output' };
				}
				case 'output': {
					return { foo: 42 };
				}
				default: {
					throw new Error(`Unexpected require "${id}"`);
				}
			}
		}
	},
	exports({ promise }) {
		return promise.then(({ foo }) => assert.strictEqual(foo, 42));
	},
	options: {
		external: ['input', 'output'],
		output: {
			generatedCode: { arrowFunctions: false },
			dynamicImportInCjs: false
		}
	}
};
