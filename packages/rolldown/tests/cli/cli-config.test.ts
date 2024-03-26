import { describe, test, assert } from 'vitest'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { loadConfig } from '../../src/cli/utils'
import { ERR_UNSUPPORTED_CONFIG_FORMAT } from '../../src/cli/errors'
import { expect } from 'vitest'

const __dirname = path.dirname(fileURLToPath(import.meta.url))

describe('loadConfig', () => {
  const RE_ERR = RegExp(ERR_UNSUPPORTED_CONFIG_FORMAT)

  test('js', async () => {
    const config = await loadConfig(
      path.resolve(__dirname, 'fixtures/rolldown.config.js'),
    )
    assert.deepEqual(config, { input: 'src/index.js' })
  })

  test('mjs', async () => {
    const config = await loadConfig(
      path.resolve(__dirname, 'fixtures/rolldown.config.mjs'),
    )
    assert.deepEqual(config, [
      { input: 'src/app1/index.js' },
      { input: 'src/app2/index.js' },
    ])
  })

  test('cjs', async () => {
    await expect(
      loadConfig(path.resolve(__dirname, 'fixtures/rolldown.config.cjs')),
    ).rejects.toThrowError(RE_ERR)
  })

  test('other format', async () => {
    await expect(
      loadConfig(path.resolve(__dirname, 'fixtures/rolldown.config.json')),
    ).rejects.toThrowError(RE_ERR)
  })

  test('not found file', async () => {
    await expect(
      loadConfig(path.join(__dirname, 'fixtures/rollup.config.js')),
    ).rejects.toThrowError(Error)
  })
})
