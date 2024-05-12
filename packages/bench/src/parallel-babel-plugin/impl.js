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
    transform(code, id) {
      const ext = nodePath.extname(id)
      if (ext === '.ts' || ext === '.tsx') {
        let now = performance.now()
        const ast = babel.parseSync(code, {
          ...partialConfig?.options,
          filename: id,
        })
        if (!ast || !partialConfig || !partialConfig.options) {
          throw new Error('failed to parse')
        }
        let diffAst = performance.now() - now
        const ret = /** @type {babel.BabelFileResult} */ (
          babel.transformFromAstSync(ast, code, {
            ...partialConfig?.options,
            filename: id,
          })
        )
        let diffTrans = performance.now() - now - diffAst
        console.log(
          id,
          'total',
          diffAst + diffTrans,
          'parse: ',
          diffAst,
          'trans: ',
          diffTrans,
        )
        return { code: /** @type {string} */ (ret.code) }
      }
    },
  }
}

export default defineParallelPluginImplementation((_options, _context) => {
  return babelPlugin()
})
