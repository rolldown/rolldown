import assert from 'node:assert'
const {foo} = require('./foo')
assert.equal(foo(), 'foo')
assert.equal(bar(), 'bar')
const {bar} = require('./bar') // This should not be hoisted
