import { defineParallelPluginImplementation } from 'rolldown/parallel-plugin'
import babel from '@babel/core'
import nodePath from 'node:path'

/** @returns {import('rolldown').Plugin} */
export const babelPlugin = () => {
  const partialConfig = babel.loadPartialConfig({
    presets: [
      ['@babel/preset-env', { bugfixes: true }],
      '@babel/preset-typescript',
    ],
    targets: 'chrome >= 80',
    sourceMaps: true,
    configFile: false,
    browserslistConfigFile: false,
  })

  return {
    name: 'parallel-babel-plugin',
    async transform(code, id) {
      const ext = nodePath.extname(id)
      if (ext === '.ts' || ext === '.tsx') {
        const ret = /** @type {babel.BabelFileResult} */ (
          await babel.transformAsync(code, {
            ...partialConfig?.options,
            filename: id,
          })
        )
        return { code: /** @type {string} */ (ret.code) }
      }
    },
  }
}

export default defineParallelPluginImplementation((_options, _context) => {
  return babelPlugin()
})
