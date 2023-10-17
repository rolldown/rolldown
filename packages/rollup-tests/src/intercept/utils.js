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

const ignoreTests = new Set(require('../ignored-tests').ignoreTests)
const onlyTests = new Set(require('../only-tests').onlyTests)

module.exports = {
  calcTestId,
  loadFailedTests,
  loadIgnoredTests: () => ignoreTests,
  loadOnlyTests: () => onlyTests,
  updateFailedTestsJson,
}
