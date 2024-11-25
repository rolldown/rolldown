import nodePath from 'node:path'
import assert from 'assert'
import join from './dist/main.js'

assert.strictEqual(join, nodePath.join)

