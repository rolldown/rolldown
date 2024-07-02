import {browser as a} from './demo-pkg/no-ext'
import {node as b} from './demo-pkg/no-ext.js'
import {browser as c} from './demo-pkg/ext'
import {browser as d} from './demo-pkg/ext.js'
import assert from 'node:assert'
assert.equal(a, 'browser')
assert.equal(b, 'node')
assert.equal(c, 'browser')
assert.equal(d, 'browser')
