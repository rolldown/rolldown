import { test } from 'vitest'
import type { TestConfig } from './types'
import { InputOptions, OutputOptions, rolldown } from 'rolldown'
import nodePath from 'node:path'
import * as fastGlob from 'fast-glob'

main()

function main() {
  const fixturesPath = nodePath.join(__dirname, 'fixtures')
  const testConfigPaths = fastGlob.sync('fixtures/**/_config.ts', {
    absolute: true,
    cwd: __dirname,
  })
  for (const testConfigPath of testConfigPaths) {
    const dirName = nodePath.relative(
      fixturesPath,
      nodePath.dirname(testConfigPath),
    )
    test(dirName, async () => {
      const testConfig: TestConfig = await import(testConfigPath).then(
        (m) => m.default,
      )
      const output = await compileFixture(
        nodePath.dirname(testConfigPath),
        testConfig,
      )
      if (testConfig.afterTest) {
        testConfig.afterTest(output)
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
