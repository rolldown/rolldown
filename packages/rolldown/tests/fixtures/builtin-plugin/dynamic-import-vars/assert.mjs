// @ts-nocheck
import assert from 'node:assert'
import { singleDir, multiDirs, noFile } from './dist/main'

singleDir('module-a-1').then((m) => {
  assert.strictEqual(m.default, 'a-1')
})

singleDir('module-a-2').then((m) => {
  assert.strictEqual(m.default, 'a-2')
})

multiDirs('a', 'module-a-1').then((m) => {
  assert.strictEqual(m.default, 'a-1')
})

multiDirs('b', 'module-b-1').then((m) => {
  assert.strictEqual(m.default, 'b-1')
})

noFile('module-c-1').catch((e) => {
  assert.strictEqual(
    e.message,
    'Unknown variable dynamic import: ./dir/c/module-c-1.js',
  )
})
