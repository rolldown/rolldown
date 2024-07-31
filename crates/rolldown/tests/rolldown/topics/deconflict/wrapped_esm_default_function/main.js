import assert from 'node:assert'
import bar from './bar'

const a = 2; // make foo `a` conflict

const { foo } = bar

assert.strictEqual(typeof foo, 'function')

require('./foo')
