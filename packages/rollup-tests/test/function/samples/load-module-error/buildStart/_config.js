module.exports = {
	description: 'buildStart hooks can use this.error',
	options: {
		plugins: [
			{
				name: 'test',
				buildStart() {
					this.error('nope');
				}
			}
		]
	},
	error: {
		code: 'PLUGIN_ERROR',
		plugin: 'test',
		message: 'nope',
		hook: 'buildStart'
	}
};
