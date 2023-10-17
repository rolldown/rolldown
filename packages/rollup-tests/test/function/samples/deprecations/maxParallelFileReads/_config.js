module.exports = {
	description: 'marks the "maxParallelFileReads" option as deprecated',
	options: {
		maxParallelFileReads: 3
	},
	error: {
		code: 'DEPRECATED_FEATURE',
		message:
			'The "maxParallelFileReads" option is deprecated. Use the "maxParallelFileOps" option instead.'
	}
};
