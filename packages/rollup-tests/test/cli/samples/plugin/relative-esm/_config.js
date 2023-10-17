module.exports = {
	description: 'ESM CLI --plugin ../relative/path',
	skipIfWindows: true,
	command: `echo 'console.log(1 ? 2 : 3);' | rollup -p "../absolute-esm/my-esm-plugin.mjs={comment: 'Relative ESM'}"`
};
