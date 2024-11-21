const require = (await import('node:module')).createRequire(import.meta.url);
const assert = require('assert')
const { '😈': devil } = require('./dist/main.js')

assert.equal(devil, 'devil')
