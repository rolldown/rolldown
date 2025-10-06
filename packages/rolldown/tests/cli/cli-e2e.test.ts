import { describe, test, it, expect } from 'vitest'
import { $, execa } from 'execa'
import { stripAnsi } from 'consola/utils'
import path from 'node:path'
import fs from 'node:fs'
import { testsDir, waitUtil } from 'rolldown-tests/utils'

function cliFixturesDir(...joined: string[]) {
  return testsDir('cli/fixtures', ...joined)
}

// remove `Finished in x ms` since it is not deterministic
// remove Ansi colors for snapshot testing
function cleanStdout(stdout: string) {
  return stripAnsi(stdout).replace(/rolldown v(?<version>\S+) Finished in \d+(\.\d+)? (s|ms|us|ns)/g, '')
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
    expect(
      cleanStdout(
        // Prevent snapshot from breaking when version changes
        ret.stdout.replace(
          /* Match `rolldown v*)` */ /rolldown\sv.*\)/,
          'rolldown VERSION)',
        ),
      ),
    ).toMatchSnapshot()
  })

  test('should not show warning with supported Node.js version', async () => {
    // NOTE: we only run tests with supported Node.js versions
    const ret = await execa`rolldown`

    expect(ret.exitCode).toBe(0)
    expect(ret.stdout).toContain('rolldown v');
    expect(ret.stdout).not.toContain('Please upgrade your Node.js version');
  })
})

describe('cli options for bundling', () => {
  it('should handle `--input` and positional args as entries', async () => {
    const cwd = cliFixturesDir('cli-multi-entries')
    const status = await $({ cwd })`rolldown 1.ts --input ./2.js`
    expect(status.exitCode).toBe(0)
    expect(cleanStdout(status.stdout)).toMatchSnapshot()
  })

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

  it('should handle pass `-s` options', async () => {
    const cwd = cliFixturesDir('cli-option-sourcemap')
    const status = await $({ cwd })`rolldown index.ts -d dist -s`
    expect(status.exitCode).toBe(0)
    expect(cleanStdout(status.stdout)).toMatchSnapshot()
  })

  it('cli default options', async () => {
    const cwd = cliFixturesDir('cli-default-option')
    const status = await $({ cwd })`rolldown -c`
    expect(status.exitCode).toBe(0)
    expect(cleanStdout(status.stdout)).toMatchSnapshot()
  })

  it('validate cli options', async () => {
    const cwd = cliFixturesDir('cli-option-object')
    try {
      await $({ cwd })`rolldown index.ts --format INCORRECT`
      expect.unreachable()
    } catch (error: any) {
      expect(error.message).matchSnapshot()
    }
  })
})

describe('config', () => {
  it('should bundle in ext-js-syntax-cjs', async () => {
    const cwd = cliFixturesDir('ext-js-syntax-cjs')
    const status = await $({ cwd })`rolldown -c rolldown.config.js`
    expect(status.exitCode).toBe(0)
    expect(cleanStdout(status.stdout)).toMatchSnapshot()
  })
  it('should bundle in cjs-config-with-replace-plugin', async () => {
    const cwd = cliFixturesDir('cjs-config-with-replace-plugin')
    const status = await $({ cwd })`rolldown -c rolldown.config.cjs`
    expect(status.exitCode).toBe(0)
    const file = path.resolve(cwd, 'dist/index.js')
    const content = fs.readFileSync(file, 'utf-8')
    expect(content).toContain("console.log(1)")
  })
  it('should not bundle in ext-js-syntax-esm', async () => {
    const cwd = cliFixturesDir('ext-js-syntax-esm')
    try {
      const _ = await $({ cwd })`rolldown -c rolldown.config.js`
    } catch (err) {
      expect(err).not.toBeUndefined()
    }
  })
  it('should allow loading ts config', async () => {
    const cwd = cliFixturesDir('ext-ts')
    const status = await $({
      cwd,
    })`rolldown -c rolldown.config.ts`
    expect(status.exitCode).toBe(0)
    expect(cleanStdout(status.stdout)).toMatchSnapshot()
  })
  it('should allow loading cts config', async () => {
    const cwd = cliFixturesDir('ext-cts')
    const status = await $({
      cwd,
    })`rolldown -c rolldown.config.cts`
    expect(status.exitCode).toBe(0)
    expect(cleanStdout(status.stdout)).toMatchSnapshot()
  })
  it('should allow loading mts config', async () => {
    const cwd = cliFixturesDir('ext-mts')
    const status = await $({
      cwd,
    })`rolldown -c rolldown.config.mts`
    expect(status.exitCode).toBe(0)
    expect(cleanStdout(status.stdout)).toMatchSnapshot()
  })
  it('should allow loading ts config with tsx', async () => {
    const cwd = cliFixturesDir('ext-ts')
    const status = await $({
      cwd,
      env: { NODE_OPTIONS: '--import=tsx' },
    })`rolldown -c rolldown.config.ts`
    expect(status.exitCode).toBe(0)
    expect(cleanStdout(status.stdout)).toMatchSnapshot()
  })

  it('should allow loading ts config with oxnode', async () => {
    const cwd = cliFixturesDir('ext-ts')
    const status = await $({
      cwd,
      env: { NODE_OPTIONS: '--import=@oxc-node/core/register' },
    })`rolldown -c rolldown.config.ts`
    expect(status.exitCode).toBe(0)
    expect(cleanStdout(status.stdout)).toMatchSnapshot()
  })

  it('should allow loading ts config from non-working dir', async () => {
    const cwd = cliFixturesDir()
    const status = await $({ cwd })`rolldown -c ./ext-ts/rolldown.config.ts`
    expect(status.exitCode).toBe(0)
    expect(cleanStdout(status.stdout)).toMatchSnapshot()
  })

  it('should allow multiply options', async () => {
    const cwd = cliFixturesDir('config-multiply-options')
    const status = await $({
      cwd,
    })`rolldown -c rolldown.config.ts`
    expect(status.exitCode).toBe(0)
    expect(cleanStdout(status.stdout)).toMatchSnapshot()
  })

  it('should allow multiply output', async () => {
    const cwd = cliFixturesDir('config-multiply-output')
    const status = await $({
      cwd,
    })`rolldown -c rolldown.config.ts`
    expect(status.exitCode).toBe(0)
    expect(cleanStdout(status.stdout)).toMatchSnapshot()
  })


  it('should support custom cli arguments', async () => {
    const cwd = cliFixturesDir('cli-with-custom-args')

    const status = await $({
      cwd,
    })`rolldown -c rolldown.config.js --customArg=customValue`
    expect(status.exitCode).toBe(0)
    expect(cleanStdout(status.stdout)).toMatchSnapshot()
  })

  it('should support environment option', async () => {
    const cwd = cliFixturesDir('cli-environment-option')

    const status = await $({
      cwd,
    })`rolldown -c rolldown.config.js --environment PRODUCTION,FOO:bar,HOST:http://localhost:4000`
    expect(status.exitCode).toBe(0)
    expect(cleanStdout(status.stdout)).toMatchSnapshot()
  })


  it('should allow multiply output + call options hook once  + call outputOptions hook', async () => {
    const cwd = cliFixturesDir('config-multiply-output-with-options-hooks')
    const status = await $({
      cwd,
    })`rolldown -c rolldown.config.ts`
    expect(status.exitCode).toBe(0)
    expect(cleanStdout(status.stdout)).toMatchSnapshot()
  })

  it('should resolve rolldown.config.cjs', async () => {
    const cwd = cliFixturesDir('cli-with-config')
    const status = await $({ cwd })`rolldown -c`
    expect(status.exitCode).toBe(0)
    expect(cleanStdout(status.stdout)).toMatchSnapshot()
  })

  it('should failed to resolve rolldown.config files', async () => {
    const cwd = cliFixturesDir('cli-without-config')
    try {
      const _ = await $({ cwd })`rolldown -c`
    } catch (err) {
      expect(err).not.toBeUndefined()
    }
  })
})

describe('watch cli', () => {
  it('call closeBundle', async () => {
    const cwd = cliFixturesDir('close-bundle')
    const status = await $({ cwd })`rolldown -c`
    expect(status.stdout).toContain('[test:closeBundle]')
    expect(status.exitCode).toBe(0)
  })

  it('should handle output options', async () => {
    const cwd = cliFixturesDir('watch-cli-option')
    const controller = new AbortController()
    execa({
      cwd,
      reject: false,
      cancelSignal: controller.signal,
    })`rolldown index.ts -d dist -w -s`
    await waitUtil(() => {
      expect(fs.existsSync(path.join(cwd, 'dist'))).toBe(true)
      expect(fs.existsSync(path.join(cwd, 'dist/index.js.map'))).toBe(true)
    })
    controller.abort()
  })

  it('should allow multiply options', async () => {
    const cwd = cliFixturesDir('config-multiply-options')
    const controller = new AbortController()
    execa({
      cwd,
      reject: false,
      cancelSignal: controller.signal,
    })`rolldown -c rolldown.config.ts -d watch-dist-options -w`
    await waitUtil(() => {
      expect(fs.existsSync(path.join(cwd, 'watch-dist-options/esm.js'))).toBe(
        true,
      )
      expect(fs.existsSync(path.join(cwd, 'watch-dist-options/cjs.js'))).toBe(
        true,
      )
    })
    controller.abort()
  })

  it('should allow multiply output', async () => {
    const cwd = cliFixturesDir('config-multiply-output')
    const controller = new AbortController()
    execa({
      cwd,
      reject: false,
      cancelSignal: controller.signal,
    })`rolldown -c rolldown.config.ts -d watch-dist-output -w`
    await waitUtil(() => {
      expect(fs.existsSync(path.join(cwd, 'watch-dist-output/esm.js'))).toBe(
        true,
      )
      expect(fs.existsSync(path.join(cwd, 'watch-dist-output/cjs.js'))).toBe(
        true,
      )
    })
    controller.abort()
  })

  it('should allow multiply output + call options hook once + call outputOptions hook', async () => {
    const cwd = cliFixturesDir('config-multiply-output-with-options-hooks')
    const status = await $({ cwd })`rolldown -c`
    expect(cleanStdout(status.stdout)).toMatchSnapshot()
    expect(status.exitCode).toBe(0)
  })

  it('should require both ROLLDOWN_WATCH and this.meta.watchMode to be false', async () => {
    const cwd = cliFixturesDir('watch-mode')
    const status = await $({ cwd })`rolldown -c`
    expect(cleanStdout(status.stdout)).toMatchSnapshot()
  })

  it('should require both ROLLDOWN_WATCH and this.meta.watchMode to be true', async () => {
    const cwd = cliFixturesDir('watch-mode')
    const status = await $({ cwd })`rolldown -w -c`
    expect(cleanStdout(status.stdout)).toMatchSnapshot()
  })
})
