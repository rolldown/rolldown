import assert from 'assert'
import main from './dist/main.mjs'

assert.equal((await main).default, 'cjs')