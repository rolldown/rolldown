import assert from 'assert'
import { esm } from './dist/main.mjs'

assert.equal(esm.default, 'esm')
