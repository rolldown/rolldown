// @ts-nocheck
import assert from 'node:assert'
import { a } from './dist/_virtual:test'

assert.strictEqual(a, 1)
