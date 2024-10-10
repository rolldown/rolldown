import assert from 'assert'
import { cjs } from './dist/main.js'

assert.equal(cjs, 'cjs')
