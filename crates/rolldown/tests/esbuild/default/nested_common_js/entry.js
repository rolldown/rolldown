import assert from 'node:assert'

function nestedScope() {
	const fn = require('./foo')
  assert(fn() === 123)
}
nestedScope()
