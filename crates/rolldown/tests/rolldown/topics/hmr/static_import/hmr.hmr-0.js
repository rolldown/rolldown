import assert from 'node:assert'
import './modules/dep-cjs'
import './modules/dep-esm'
import cjsDefault from './modules/dep-cjs-default'
import esmDefault from './modules/dep-esm-default'
import { named as cjsNamed } from './modules/dep-cjs-named'
import { named as esmNamed } from './modules/dep-esm-named'
import * as cjsNamespace from './modules/dep-cjs-namespace'
import * as esmNamespace from './modules/dep-esm-namespace'
import './modules/new-dep-cjs'
import './modules/new-dep-esm'

assert.strictEqual(cjsDefault, 'cjs-default')
assert.strictEqual(esmDefault, 'esm-default')
assert.strictEqual(cjsNamed, 'cjs-named')
assert.strictEqual(esmNamed, 'esm-named')
assert.deepStrictEqual(cjsNamespace, {
  value: 'cjs-namespace',
  default: { value: 'cjs-namespace' }
})
assert.deepStrictEqual(esmNamespace, { value: 'esm-namespace' })

import.meta.hot.accept((mod) => {
  if (mod) {
    console.log('.hmr', mod.foo)
  }
})
