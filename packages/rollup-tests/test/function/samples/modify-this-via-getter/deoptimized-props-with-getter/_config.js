module.exports = {
	description: 'handles fully deoptimized objects',
	context: {
		require() {
			return { unknown: 'prop' };
		}
	},
	options: {
		external: ['external']
	}
};
