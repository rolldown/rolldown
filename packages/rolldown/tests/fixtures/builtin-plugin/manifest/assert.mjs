// @ts-nocheck
import assert from 'node:assert'
import Manifest from './dist/manifest.json'

assert(Manifest['asset.txt'].file.match(/^assets\/asset-[\w-]+.txt$/))
assert.strictEqual(Manifest['asset.txt'].src, 'asset.txt')

assert(Manifest['chunk.js'].file.match(/^chunk-[\w-]+.js$/))
assert.strictEqual(Manifest['chunk.js'].name, 'chunk')
assert.strictEqual(Manifest['chunk.js'].src, 'chunk.js')
assert.strictEqual(Manifest['chunk.js'].is_dynamic_entry, true)

assert.deepStrictEqual(Manifest['main.js'], {
  file: 'main.js',
  name: 'main',
  src: 'main.js',
  is_entry: true,
  dynamic_imports: ['chunk.js'],
})
