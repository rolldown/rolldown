export default {
	input: 'main.js',
	output: [
		{
			dir: '_actual/dist1',
			format: 'cjs'
		},
		{
			dir: '_actual/dist2',
			format: 'es'
		}
	],
	plugins: [
		{
			generateBundle() {
				this.emitFile({ type: 'asset', fileName: 'myfile', source: 'abc' });
			}
		}
	]
};
