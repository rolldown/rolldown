import assert from 'node:assert'

function nestedScope() {
	const fn = require('./foo')
  assert.equal(fn(), 123)
}
nestedScope()
