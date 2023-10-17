const assert = require('node:assert');

const COLOR = '\u001B[36m\u001B[1m';
const STANDARD = '\u001B[22m\u001B[39m';

module.exports = {
	description: 'bundles multiple files to stdout while adding file names',
	skipIfWindows: true,
	command:
		'node wrapper.js -i main1.js -i main2.js -f es ' +
		`-p '{buildStart(){this.emitFile({type: "asset",source:"Hello"})}}'`,
	env: { FORCE_COLOR: '1', TERM: 'xterm' },
	result(code) {
		assert.equal(
			code,
			'\n' +
				`${COLOR}//→ main1.js:${STANDARD}\n` +
				"console.log('main1');\n" +
				'\n' +
				`${COLOR}//→ main2.js:${STANDARD}\n` +
				"console.log('main2');\n" +
				'\n' +
				`${COLOR}//→ assets/asset-185f8db3:${STANDARD}\n` +
				'Hello'
		);
	}
};
