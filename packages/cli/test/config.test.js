import assert from 'node:assert'
import { describe, test } from 'node:test'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { loadConfig } from '../lib/config.js'

const __dirname = path.dirname(fileURLToPath(import.meta.url))

describe('loadConfig', () => {
  test('js', () => {
    const config = loadConfig(
      path.resolve(__dirname, 'fixtures/rolldown.config.js'),
    )
    assert.deepEqual(config, { default: { input: 'src/index.js' } })
  })

  test('mjs', () => {
    const config = loadConfig(
      path.resolve(__dirname, 'fixtures/rolldown.config.mjs'),
    )
    assert.deepEqual(config, {
      default: [{ input: 'src/app1/index.js' }, { input: 'src/app2/index.js' }],
    })
  })

  test('ts', () => {
    const { default: config } = loadConfig(
      path.resolve(__dirname, 'fixtures/rolldown.config.ts'),
    )
    assert(config.input === 'src/index.ts')

    const plugin = config.plugins.find(
      (plugin) => plugin.name === 'test-plugin',
    )
    assert(typeof plugin.transform === 'function')
  })

  test('cjs', () => {
    assert.throws(() => {
      loadConfig(path.resolve(__dirname, 'fixtures/rolldown.config.cjs'))
    }, /Unsupported config format. please use '.js', '.mjs' and '.ts' format/)
  })

  test('other format', () => {
    assert.throws(() => {
      loadConfig(path.join(__dirname, 'fixtures/rolldown.config.json'))
    }, /Unsupported config format. please use '.js', '.mjs' and '.ts' format/)
  })
})
