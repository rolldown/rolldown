const assert = require('assert')
const { '😈': devil } = require('./dist/main.js')

assert.equal(devil, 'devil')
