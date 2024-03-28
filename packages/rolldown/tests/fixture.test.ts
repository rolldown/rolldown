import { test } from 'vitest'
import type { TestConfig } from './src/types'
import { InputOptions, OutputOptions, rolldown } from 'rolldown'
import nodePath from 'node:path'
import * as fastGlob from 'fast-glob'
import { loadTestConfig } from '@tests/utils'

main()

function main() {
  const fixturesPath = nodePath.join(__dirname, 'fixtures')
  const testConfigPaths = fastGlob.sync('fixtures/**/_config.ts', {
    absolute: true,
    cwd: __dirname,
  })
  for (const testConfigPath of testConfigPaths) {
    const dirPath = nodePath.relative(
      fixturesPath,
      nodePath.dirname(testConfigPath),
    )
    test(dirPath, async (ctx) => {
      const testConfig = await loadTestConfig(testConfigPath)
      if (testConfig.skip) {
        ctx.skip()
        return
      }

      try {
        const output = await compileFixture(
          nodePath.dirname(testConfigPath),
          testConfig,
        )
        if (testConfig.afterTest) {
          testConfig.afterTest(output)
        }
      } catch (err) {
        throw new Error(`Failed in ${testConfigPath}`, { cause: err })
      }
    })
  }
}

async function compileFixture(fixturePath: string, config: TestConfig) {
  let outputOptions: OutputOptions = config.config?.output ?? {}
  delete config.config?.output
  outputOptions = {
    dir: outputOptions.dir ?? nodePath.join(fixturePath, 'dist'),
    ...outputOptions,
  }

  const inputOptions: InputOptions = {
    input: config.config?.input ?? nodePath.join(fixturePath, 'main.js'),
    ...config.config,
  }
  const build = await rolldown(inputOptions)
  return await build.write(outputOptions)
}
