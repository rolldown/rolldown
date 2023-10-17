module.exports = {
	description: 'mixed synthetic named exports 2',
	options: {
		input: ['main.js', 'main2.js'],
		plugins: [
			{
				transform(code, id) {
					if (id.endsWith('dep1.js') || id.endsWith('dep2.js')) {
						return {
							code,
							syntheticNamedExports: true
						};
					}
					return null;
				}
			}
		]
	}
};
