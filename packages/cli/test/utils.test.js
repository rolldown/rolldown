import assert from 'node:assert'
import { describe, test } from 'node:test'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { getPackageJSON } from '../lib/utils.js'
import { ERR_CLI_META_DATA } from '../lib/errors.js'

const __dirname = path.dirname(fileURLToPath(import.meta.url))

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
      }, new RegExp(ERR_CLI_META_DATA))
    })

    test('failure', () => {
      assert.throws(() => {
        // no such file or directory
        getPackageJSON(path.resolve(__dirname, '../..'))
      }, Error)
    })
  })
})
