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
  const content = fs.readFileSync(path.join(__dirname, '../ignored-by-supported-features.md'), 'utf-8')
  const matches = content.match(/- (.*)/g)
  if (matches) {
    for (const id of matches) {
      unsupportedIgnoredTests.push(id.replace('- ', '').trim())
    }
  }
  return unsupportedIgnoredTests
}

const ignoreTests = new Set(require('../ignored-tests').ignoreTests)
const onlyTests = new Set(require('../only-tests').onlyTests)
const unsupportedFeaturesIgnoredTests = loadUnsupportedFeaturesIgnoredTests()

module.exports = {
  calcTestId,
  loadFailedTests,
  loadIgnoredTests: () => ignoreTests,
  loadOnlyTests: () => onlyTests,
  updateFailedTestsJson,
  unsupportedFeaturesIgnoredTests,
}
