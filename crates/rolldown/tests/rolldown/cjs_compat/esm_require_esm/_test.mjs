import assert from 'assert'
import { esm } from './dist/main.js'

assert.equal(esm.default, 'esm')
