const {
  loadFailedTests,
  calcTestId,
  updateFailedTestsJson,
  shouldIgnoredTest,
  status
} = require('./utils')
const fs = require('node:fs')
const path = require('node:path')

const alreadyFailedTests = new Set(loadFailedTests())

beforeEach(function skipAlreadyFiledTests() {
  if (!this.currentTest) {
    throw new Error('No current test')
  }
  const id = calcTestId(this.currentTest)
  // status.total += 1

  // if (!onlyTests.has(id)) {
  //   this.currentTest?.skip()
  // }

  if (shouldIgnoredTest(id)) {
    // status.ignored += 1
    this.currentTest?.skip()
  }

  if (alreadyFailedTests.has(id)) {
    status.skipFailed += 1
    this.currentTest.skip()
  }

  // Easy way to find the test id in the logs
  console.log(id)
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
  const testId = calcTestId(this.currentTest)
  const state = this.currentTest.state
  if (state === 'failed') {
    status.failed += 1
    alreadyFailedTests.add(testId)
  } else if (state === 'passed') {
    status.passed += 1
  }
})

after(function printStatus() {
  updateFailedTestsJson(alreadyFailedTests)
  fs.writeFileSync(path.join(__dirname, '../status.json'), JSON.stringify(status, null, 2))
  writeTestStatusToMarkdown()
  // enforce exit process to avoid Rust process is not exit.
  process.exit(0)
})

function writeTestStatusToMarkdown() {
  let markdown = '|  | number |\n|----| ---- |\n'
  const statusKeys = /** @type {Array<keyof typeof status>} */ (Object.keys(status))
  for (const key of statusKeys) {
    markdown += `| ${key} | ${status[key]} |\n`
  }
  fs.writeFileSync(path.join(__dirname, '../status.md'), markdown)
}
