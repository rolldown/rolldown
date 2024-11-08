/**
 * Run `npx mocha found-tree-shaking-not-align.js`
 */
const fs = require('node:fs')
const path = require('node:path')
const {
	runTestSuiteWithSamples,
} = require('../utils.js');

const ignoredTreeshakingTests = []
const testDirectory = path.resolve(__dirname, '../../../../rollup/test/form/samples')

runTestSuiteWithSamples(
	'form',
	testDirectory,
	/**
	 * @param {import('../types').TestConfigForm} config
	 */
	(directory, config) => {
        const content = fs.readFileSync(directory + '/main.js', 'utf-8');
        if (content.includes('// removed') || content.includes(`console.log('removed')`) || content.includes('const unused = ')  || content.includes('const removed') || config.description.includes('Tree-shake') || config.description.includes('tree-shake') || directory.includes('tree-shake') || directory.includes('treeshakes') || directory.includes('side-effect') || config.description.includes('side-effect') || content.includes('/*#__NO_SIDE_EFFECTS__*/') || directory.includes('skips-dead-branches') 
        || content.includes('tree-shake') || content.includes('side-effect')) {
            const testPath = directory.replace(testDirectory, '').replaceAll('/', '@')
            const isSingleFormatTest = fs.existsSync(directory + '/_expected.js');
            if (isSingleFormatTest) {
                ignoredTreeshakingTests.push('rollup@form' + testPath + ': ' + config.description)
            } else {
                ignoredTreeshakingTests.push('rollup@form' + testPath + ': ' + config.description + '@generates es')
            }
        }
	}
);

fs.writeFileSync(path.join(__dirname, '../../src/ignored-treeshaking-tests.json'), JSON.stringify(ignoredTreeshakingTests, null, 2))