const { assertIncludes } = require('../../../utils.js');

module.exports = {
	description: 'warns when there are multiple missing globals',
	command: 'rollup -c',
	stderr: stderr =>
		assertIncludes(
			stderr,
			'(!) Missing global variable names\n' +
				'https://rollupjs.org/configuration-options/#output-globals\n' +
				'Use "output.globals" to specify browser global variable names corresponding to external modules:\n' +
				'external1 (guessing "foo1")\n' +
				'external2 (guessing "foo2")\n' +
				'external3 (guessing "foo3")'
		)
};
