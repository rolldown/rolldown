module.exports = {
	description: 'escapes reserved names used as props',
	options: {
		external: ['external', 'external2', 'externalDefaultOnly'],
		output: {
			exports: 'named',
			generatedCode: { reservedNamesAsProps: false },
			interop(id) {
				if (id === 'externalDefaultOnly') return 'defaultOnly';
				return 'auto';
			},
			name: 'bundle'
		},
		plugins: [
			{
				transform() {
					return { syntheticNamedExports: true };
				}
			}
		]
	}
};
