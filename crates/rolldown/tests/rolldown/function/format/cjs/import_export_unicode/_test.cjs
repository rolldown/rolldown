const assert = require('assert')
const { '😈': devil } = require('./dist/main.cjs')

assert.equal(devil, 'devil')