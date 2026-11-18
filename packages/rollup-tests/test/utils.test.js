const assert = require('node:assert/strict');
const { compareError } = require('./utils.js');

describe('compareError', () => {
	const expected = {
		code: 'PLUGIN_ERROR',
		hook: 'renderStart',
		message: 'rendering failed',
		plugin: 'test'
	};

	for (const assertionFirst of [true, false]) {
		it(`rethrows an assertion ${assertionFirst ? 'before' : 'after'} a matching error`, () => {
			const assertion = new assert.AssertionError({ message: 'renderError assertion failed' });
			const matchingError = { ...expected };
			const errors = assertionFirst ? [assertion, matchingError] : [matchingError, assertion];

			assert.throws(() => compareError({ errors }, expected), error => error === assertion);
		});
	}

	it('accepts a matching error alongside another ordinary error', () => {
		assert.doesNotThrow(() =>
			compareError({ errors: [new Error('cleanup failed'), { ...expected }] }, expected)
		);
	});
});
