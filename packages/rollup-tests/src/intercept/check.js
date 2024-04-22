const { loadFailedTests, calcTestId, loadIgnoredTests } = require('./utils')
const expectedStatus = require('../status.json');
const alreadyFailedTests = new Set(loadFailedTests())

const ignoreTests = loadIgnoredTests()

const status = {
  // total: 0,
  failed: 0,
  skipFailed: 0,
  // ignored: 0,
  skipped: 0,
  passed: 0,
}

/**
 * @type {Set<string>}
 */
const failedTests = new Set()

beforeEach(function skipAlreadyFiledTests() {
  if (!this.currentTest) {
    throw new Error('No current test')
  }
  // status.total += 1
  const id = calcTestId(this.currentTest)

  if (ignoreTests.has(id)) {
    // status.ignored += 1
    this.currentTest?.skip()
  }

  if (alreadyFailedTests.has(id)) {
    status.skipFailed += 1
    this.currentTest.skip()
  }

  // capture the current test reference
  const currentTest = this.currentTest
  setTimeout(() => {
    if (currentTest.state !== 'passed' && currentTest.state !== 'failed') {
      // Emit a custom error to make it easier to find the test that timed out.
      currentTest.callback?.(new Error(`Test timed out: [${id}]`))
    }
  }, 500)
})

afterEach(function updateStatus() {
  if (!this.currentTest) {
    throw new Error('No current test')
  }
  const state = this.currentTest.state
  if (state === 'failed') {
    console.error(this.currentTest.err)
    status.failed += 1
    failedTests.add(calcTestId(this.currentTest))
  } else if (state === 'passed') {
    status.passed += 1
  }
})

after(function printStatus() {
  const sorted = [...failedTests].sort()
  console.log('failures', JSON.stringify(sorted, null, 2))
  console.table(status)
  if (status.failed > 0) {
    // enforce exit process to avoid rust process is not exit.
    process.exit(1)
  } else {
    if (expectedStatus.skipFailed !== status.skipFailed || expectedStatus.passed !== status.passed) {
      console.log('expected', expectedStatus)
      console.log('actual', status)
      throw new Error('The rollup test status file is not updated. Please run `just test-node rollup --update` to update it.')
    }
    process.exit(0)
  }
})
