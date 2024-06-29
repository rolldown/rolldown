import {fn} from './foo'
import assert from 'node:assert'
(() => {
	assert.equal(fn(), 123)
})()
