/**
 * Run `npx mocha found-tree-shaking-not-align.js`
 */
const fs = require('node:fs')
const path = require('node:path')
const {
	runTestSuiteWithSamples,
} = require('../utils.js');
const {
	shouldIgnoreTestExceptTreeshaking,
} = require('../../src/intercept/utils.js');

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
        if (content.includes('// removed') || content.includes(`console.log('removed')`) || content.includes('const removed') || included(directory) || included(config.description) || included(content)) {
            const testPath = directory.replace(testDirectory, '').replaceAll('/', '@').replaceAll('\\', '@')
            const isSingleFormatTest = fs.existsSync(directory + '/_expected.js');
            const testId = isSingleFormatTest
                ? 'rollup@form' + testPath + ': ' + config.description
                : 'rollup@form' + testPath + ': ' + config.description + '@generates es'
            if (!shouldIgnoreTestExceptTreeshaking(testId)) {
                ignoredTreeshakingTests.push(testId)
            }
        }
	}
);

function included(str) {
    return str.includes('Tree-shake') || str.includes('tree-shake') || str.includes('tree-shaking')  || str.includes('treeshake') || str.includes('side-effect') || str.includes('/*#__NO_SIDE_EFFECTS__*/') || str.includes('skips-dead-branches')
     || str.includes('deoptimizations') || str.includes('deoptimize') || str.includes('removes an empty') || str.includes('unused')
}

fs.writeFileSync(path.join(__dirname, '../../src/ignored-treeshaking-tests.json'), JSON.stringify(ignoredTreeshakingTests, null, 2))
