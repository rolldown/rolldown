import assert from 'assert'
import main from './dist/main.js'

assert.equal((await main).default, 'cjs')
