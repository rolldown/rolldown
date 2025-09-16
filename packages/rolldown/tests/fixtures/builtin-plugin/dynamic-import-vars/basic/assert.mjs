// @ts-nocheck
import assert from 'node:assert'
import { singleDir, multiDirs, noFile, withAlias, withIgnoreTag } from './dist/main'

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

withAlias('module-a-1').then((m) => {
  assert.strictEqual(m.default, 'a-1')
})

// Vitest transforms `import(..)` to `vite_ssr_dynamic_import(..)`
assert.strictEqual(withIgnoreTag.toString().match(/\(\s*\/\*\s*@vite-ignore\s*\*\/\s*`([^`]+)`\s*\)/)[1], './dir/a/${name}.js');