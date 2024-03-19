import assert from 'node:assert'
import { describe, test } from 'node:test'
import path from 'node:path'
import { getPackageJSON } from '../lib/utils.js'

const __dirname = path.dirname(new URL(import.meta.url).pathname)

describe('utils', () => {
  describe('getPackageJSON', () => {
    test('success', () => {
      const pkg = getPackageJSON(path.resolve(__dirname, '..'))
      assert(pkg.version != null)
      assert(pkg.description != null)
    })

    test('invalid package.json', () => {
      assert.throws(() => {
        getPackageJSON(path.resolve(__dirname, './fixtures'))
      }, /cli meta data error/)
    })

    test('failure', () => {
      assert.throws(() => {
        // no such file or directory
        getPackageJSON(path.resolve(__dirname, '../..'))
      }, Error)
    })
  })
})
