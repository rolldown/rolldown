const assert = require('node:assert');

module.exports = {
	description:
		'adds Symbol.toStringTag property to dynamic imports of entry chunks with default export mode',
	options: {
		input: ['main', 'foo'],
		output: {
			generatedCode: { symbols: true }
		}
	},
	async exports(exports) {
		const foo = await exports;
		assert.strictEqual(foo[Symbol.toStringTag], 'Module');
		assert.strictEqual(Object.prototype.toString.call(foo), '[object Module]');
		assert.strictEqual(foo.default, 42);
	}
};
