import { describe, test, it, expect } from 'vitest'
import { execSync } from 'node:child_process'
import { $ } from 'execa'

import { projectDir, testsDir } from '@tests/utils'

function cliFixturesDir(...joined: string[]) {
  return testsDir('cli/fixtures', ...joined)
}

describe('should not hang after running', () => {
  test.skip('basic', async () => {
    const cwd = cliFixturesDir('no-config')
    const _ret = execSync('rolldown', { cwd })
  })
})

describe('args', () => {
  it('should render help message for empty args', async () => {
    try {
      execSync('rolldown', {
        cwd: projectDir(),
        encoding: 'utf-8',
      })
    } catch (err: any) {
      expect(err.message).toMatchSnapshot()
    }
  })
})

describe('config', () => {
  describe('no package.json', () => {
    it('should bundle in ext-js-syntax-cjs', async () => {
      const cwd = cliFixturesDir('ext-js-syntax-cjs')
      const status = await $({ cwd })`rolldown -c rolldown.config.js`
      expect(status.exitCode).toBe(0)
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
