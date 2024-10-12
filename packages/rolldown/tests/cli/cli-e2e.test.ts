import { describe, test, it, expect } from 'vitest'
import { $, execa } from 'execa'
import { stripAnsi } from 'consola/utils'
import { testsDir } from '@tests/utils'

function cliFixturesDir(...joined: string[]) {
  return testsDir('cli/fixtures', ...joined)
}

// remove `Finished in x ms` since it is not deterministic
// remove Ansi colors for snapshot testing
function cleanStdout(stdout: string) {
  return stripAnsi(stdout).replace(/Finished in \d+(\.\d+)? ms/g, '')
}

describe('should not hang after running', () => {
  test.skip('basic', async () => {
    const cwd = cliFixturesDir('no-config')
    const _ret = execa(`rolldown`, { cwd })
  })
})

describe('basic arguments', () => {
  test('should render help message for empty args', async () => {
    const ret = await execa`rolldown`

    expect(ret.exitCode).toBe(0)
    expect(cleanStdout(ret.stdout)).toMatchSnapshot()
  })
})

describe('cli options for bundling', () => {
  it('should handle single boolean option', async () => {
    const cwd = cliFixturesDir('cli-option-boolean')
    const status = await $({ cwd })`rolldown index.ts --minify -d dist`
    expect(status.exitCode).toBe(0)
    expect(cleanStdout(status.stdout)).toMatchSnapshot()
  })

  it('should handle single boolean short options', async () => {
    const cwd = cliFixturesDir('cli-option-short-boolean')
    const status = await $({ cwd })`rolldown index.ts -m -d dist`
    expect(status.exitCode).toBe(0)
    expect(cleanStdout(status.stdout)).toMatchSnapshot()
  })

  it('should handle single string options', async () => {
    const cwd = cliFixturesDir('cli-option-string')
    const status = await $({
      cwd,
    })`rolldown index.ts --format cjs -d dist`
    expect(status.exitCode).toBe(0)
    expect(cleanStdout(status.stdout)).toMatchSnapshot()
  })

  it('should handle single array options', async () => {
    const cwd = cliFixturesDir('cli-option-array')
    const status = await $({
      cwd,
    })`rolldown index.ts --external node:path --external node:url -d dist`
    expect(status.exitCode).toBe(0)
    expect(cleanStdout(status.stdout)).toMatchSnapshot()
  })

  it('should handle single object options', async () => {
    const cwd = cliFixturesDir('cli-option-object')
    const status = await $({
      cwd,
    })`rolldown index.ts --module-types .123=text --module-types notjson=json --module-types .b64=base64 -d dist`
    expect(status.exitCode).toBe(0)
    expect(cleanStdout(status.stdout)).toMatchSnapshot()
  })

  it('should handle negative boolean options', async () => {
    const cwd = cliFixturesDir('cli-option-no-external-live-bindings')
    const status = await $({
      cwd,
    })`rolldown index.ts --format iife --external node:fs --no-external-live-bindings`
    expect(status.exitCode).toBe(0)
    expect(cleanStdout(status.stdout)).toMatchSnapshot()
  })
})

describe('config', () => {
  describe('no package.json', () => {
    it('should bundle in ext-js-syntax-cjs', async () => {
      const cwd = cliFixturesDir('ext-js-syntax-cjs')
      const status = await $({ cwd })`rolldown -c rolldown.config.js`
      expect(status.exitCode).toBe(0)
      expect(cleanStdout(status.stdout)).toMatchSnapshot()
    })
    it('should not bundle in ext-js-syntax-esm', async () => {
      const cwd = cliFixturesDir('ext-js-syntax-esm')
      try {
        const _ = await $({ cwd })`rolldown -c rolldown.config.js`
      } catch (err) {
        expect(err).not.toBeUndefined()
      }
    })
  })
})
