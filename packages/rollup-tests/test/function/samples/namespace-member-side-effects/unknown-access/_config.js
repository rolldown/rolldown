module.exports = {
	description: 'respects side effects when accessing unknown namespace members',
	options: {
		external: ['external'],
		treeshake: { tryCatchDeoptimization: false }
	}
};
