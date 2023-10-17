module.exports = {
	description: 'imports external namespaces',
	options: {
		external: ['foo', 'bar'],
		output: {
			globals: { foo: 'foo', bar: 'bar' }
		}
	}
};
