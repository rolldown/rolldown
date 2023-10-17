const { assertIncludes } = require('../../../utils.js');

module.exports = {
	description: 'warns for multiple circular dependencies',
	command: 'rollup -c',
	stderr: stderr =>
		assertIncludes(
			stderr,
			'(!) Circular dependencies\n' +
				'main.js -> dep1.js -> main.js\n' +
				'main.js -> dep2.js -> main.js\n' +
				'main.js -> dep3.js -> main.js\n' +
				'...and 3 more\n' +
				''
		)
};
