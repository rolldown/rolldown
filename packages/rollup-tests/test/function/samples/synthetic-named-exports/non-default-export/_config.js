const assert = require('node:assert');

module.exports = {
	description: 'supports providing a named export to generate synthetic exports',
	options: {
		plugins: [
			{
				transform(code, id) {
					if (id.endsWith('dep.js')) {
						return { code, syntheticNamedExports: '__synthetic' };
					}
				},
				moduleParsed({ id, syntheticNamedExports }) {
					if (id.endsWith('dep.js')) {
						assert.strictEqual(syntheticNamedExports, '__synthetic');
					}
				}
			}
		]
	},
	exports(exports) {
		assert.deepStrictEqual(exports, {
			dep: 'default',
			doesNotExist: undefined,
			exists: 'exists',
			synthetic: 'synthetic'
		});
	}
};
