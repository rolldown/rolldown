// eslint-disable-next-line import/no-unresolved
import type * as rollup from '../../dist/rollup';

// Plugin API
interface Options {
	extensions?: string | string[];
}

const plugin: rollup.PluginImpl<Options> = (options = {}) => {
	const _extensions = options.extensions || ['.js'];
	return {
		name: 'my-plugin',
		resolveId: {
			handler(source, _importer, _options) {
				const _s: number = source;
			}
		}
	};
};

plugin();
plugin({ extensions: ['.js', 'json'] });

const _pluginHooks: rollup.Plugin = {
	buildStart: {
		handler() {},
		sequential: true
	},
	async load(id) {
		const _index: number = id;
		await this.resolve('rollup');
	},
	name: 'test',
	resolveId: {
		async handler(source, _importer, _options) {
			await this.resolve('rollup');
			const _s: number = source;
		},
		sequential: true
	}
};

const _amdOutputOptions: rollup.OutputOptions['amd'][] = [
	{},
	{
		id: 'a'
	},
	{
		autoId: false,
		id: 'a'
	},
	{
		autoId: true,
		basePath: 'a'
	},
	{
		autoId: true
	},
	{
		autoId: false
	},
	{
		autoId: false,
		basePath: '',
		id: 'a'
	},
	{
		autoId: true,
		id: 'a'
	},
	{
		basePath: '',
		id: 'a'
	},
	{
		basePath: ''
	}
];
