module.exports = {
	description: 'throws for absolute paths as input names',
	options: {
		input: { '/test': 'main.js' }
	},
	generateError: {
		code: 'VALIDATION_ERROR',
		message:
			'Invalid substitution "/test" for placeholder "[name]" in "output.entryFileNames" pattern, can be neither absolute nor relative path.'
	}
};
