module.exports = {
	description: 'external re-exports aliasing',
	options: {
		output: {
			globals: { d: 'd' },
			name: 'reexportsAliasingExternal'
		},
		external: ['d']
	}
};
