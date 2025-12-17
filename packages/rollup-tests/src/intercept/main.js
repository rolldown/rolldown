const hasGrep = process.argv.some(arg => arg === '--grep' || arg === '-g')
const hasUpdate = process.argv.includes('--update')

if (hasUpdate && hasGrep) {
  throw new Error('Cannot use --update with --grep')
}

if (hasUpdate) {
  require('./update-test-status')
} else {
  require('./check')
}
