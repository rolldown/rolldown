module.exports = {
	description: 'throws for anonymous plugins adding to the cache',
	options: {
		plugins: {
			buildStart() {
				this.cache.set('asdf', 'asdf');
			}
		}
	},
	error: {
		code: 'PLUGIN_ERROR',
		hook: 'buildStart',
		message:
			'A plugin is trying to use the Rollup cache but is not declaring a plugin name or cacheKey.',
		plugin: 'at position 1',
		pluginCode: 'ANONYMOUS_PLUGIN_CACHE'
	}
};
