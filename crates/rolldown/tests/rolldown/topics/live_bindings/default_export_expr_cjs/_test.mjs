const assert = require('node:assert')
const main = require('./dist/main.js')

main.reset()
assert.strictEqual(main.default, 0)
inc()
assert.strictEqual(main.default, 0)
