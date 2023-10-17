const assert = require('node:assert');
let referenceId;

module.exports = {
	description: 'supports emitting chunks as side effect of the manual chunks option',
	options: {
		output: {
			manualChunks: { foo: ['manual.js'] },
			assetFileNames: '[name]-[hash][extname]'
		},
		plugins: {
			transform(code, id) {
				if (id.endsWith('manual.js')) {
					referenceId = this.emitFile({ type: 'asset', name: 'emitted.txt', source: 'emitted' });
				}
			},
			generateBundle() {
				assert.strictEqual(this.getFileName(referenceId), 'emitted-f57bfbce.txt');
			}
		}
	}
};
