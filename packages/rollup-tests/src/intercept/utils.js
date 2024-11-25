const fs = require('fs')
const path = require('path')

/**
 * @param {Mocha.Test} test
 * @returns {string}
 */
function calcTestId(test) {
  const paths = test.titlePath()
  return paths.join('@')
}

const failedTestsPath = path.join(__dirname, '../failed-tests.json')

/**
 * @returns {string[]}
 */
function loadFailedTests() {
  if (fs.existsSync(failedTestsPath)) {
    const failedTests = JSON.parse(fs.readFileSync(failedTestsPath, 'utf-8'))
    return failedTests
  }
  return []
}

/**
 * @param {Set<string>} failuresInThisRound
 */
function updateFailedTestsJson(failuresInThisRound) {
  const sorted = [...failuresInThisRound].sort()
  const formatted = JSON.stringify(sorted, null, 2)
  fs.writeFileSync(path.join(__dirname, '../failed-tests.json'), formatted)
}

function loadUnsupportedFeaturesIgnoredTests() {
  const unsupportedIgnoredTests = []
  const content = fs.readFileSync(path.join(__dirname, '../ignored-by-unsupported-features.md'), 'utf-8')
  const matches = content.match(/- (.*)/g)
  if (matches) {
    for (const id of matches) {
      unsupportedIgnoredTests.push(id.replace('- ', '').trim())
    }
  }
  return unsupportedIgnoredTests
}

const ignoredTests = new Set(require('../ignored-tests').ignoreTests)
const onlyTests = new Set(require('../only-tests').onlyTests)
const unsupportedFeaturesIgnoredTests = loadUnsupportedFeaturesIgnoredTests()
const ignoredTreeshakingTests = new Set(require('../ignored-treeshaking-tests'))
const ignoredSnapshotDifferentTests = new Set(require('../ignored-passed-snapshot-different-tests'))

// The windows has special test, so here total is different at different platform.
const status = {
  // total: 0,
  failed: 0,
  skipFailed: 0,
  ignored: ignoredTests.size,
  'ignored(unsupported features)': unsupportedFeaturesIgnoredTests.length,
  'ignored(treeshaking)': ignoredTreeshakingTests.size,
  'ignored(behavior passed, snapshot different)': ignoredSnapshotDifferentTests.size,
  passed: 0,
}

/**
 * @param {string} id
 */
function shouldIgnoredTest(id) {
  return ignoredTests.has(id) || ignoredSnapshotDifferentTests.has(id) || ignoredTreeshakingTests.has(id) || unsupportedFeaturesIgnoredTests.find((test) => test.includes(id))
}

module.exports = {
  calcTestId,
  loadFailedTests,
  loadIgnoredTests: () => ignoredTests,
  loadOnlyTests: () => onlyTests,
  updateFailedTestsJson,
  shouldIgnoredTest,
  status
}
