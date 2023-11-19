import assert from 'assert'
import { cjs } from './dist/main.mjs'

assert.equal(cjs, 'cjs')
