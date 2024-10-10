const assert = require('node:assert')
const main = require('./dist/main.js')

main.reset()
assert.strictEqual(main.default, 0)
main.inc()
assert.strictEqual(main.default, 1)
