const path = require('node:path');
const ID_MAIN = path.join(__dirname, 'main.js');
const ID_EMPTY = path.join(__dirname, 'empty.js');

module.exports = {
	description: 'replaces missing namespace members with undefined and warns about them',
	warnings: [
		{
			binding: 'foo',
			code: 'MISSING_EXPORT',
			exporter: ID_EMPTY,
			id: ID_MAIN,
			message: '"foo" is not exported by "empty.js", imported by "main.js".',
			url: 'https://rollupjs.org/troubleshooting/#error-name-is-not-exported-by-module',
			pos: 61,
			loc: {
				column: 25,
				file: ID_MAIN,
				line: 3
			},
			frame: `
				1: import * as mod from './empty.js';
				2:
				3: assert.equal( typeof mod.foo, 'undefined' );
				                            ^`
		}
	]
};
