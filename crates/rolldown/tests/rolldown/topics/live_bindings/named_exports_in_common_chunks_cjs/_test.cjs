const assert = require('assert')
const main = require('./dist/main.js')

main.reset()
assert.strictEqual(main.count, 0)
main.inc()
assert.strictEqual(main.count, 1)
main.inc()
assert.strictEqual(main.count, 2)
