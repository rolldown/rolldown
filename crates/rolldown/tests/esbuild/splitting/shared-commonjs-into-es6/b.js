import assert from 'node:assert'
const {foo} = require("./shared.js")
assert.equal(foo, 123)
