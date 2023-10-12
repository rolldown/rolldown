const { loadFailedTests, calcTestId, loadIgnoredTests } = require('./utils')

const alreadyFailedTests = new Set(loadFailedTests())

const ignoreTests = loadIgnoredTests()

const status = {
  total: 0,
  failed: 0,
  skipFailed: 0,
  ignored: 0,
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
  status.total += 1
  const id = calcTestId(this.currentTest)
  if (alreadyFailedTests.has(id)) {
    status.skipFailed += 1
    this.currentTest.skip()
  }

  if (ignoreTests.has(id)) {
    status.ignored += 1
    this.currentTest?.skip()
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
  // enforce exit process to avoid rust process is not exit.
  process.exit(status.failed > 0 ? 1 : 0)
})
