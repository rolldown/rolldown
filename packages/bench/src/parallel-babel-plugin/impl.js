import { defineThreadSafePluginImplementation } from 'rolldown/thread-safe-plugin'
import nodePath from 'node:path'
import swc from '@swc/core'

/** @returns {import('rolldown').Plugin} */
export const babelPlugin = () => {
  return {
    name: 'parallel-babel-plugin',
    async transform(code, id) {
      const ext = nodePath.extname(id)
      if (ext === '.ts' || ext === '.tsx') {
        const ret = /** @type {swc.Output} */ (
          await swc.transform(code, {
            filename: id,
            jsc: {
              parser: {
                syntax: 'typescript',
              },
            },
            env: {
              targets: 'chrome >= 80',
              bugfixes: true,
            },
            sourceMaps: true,
            configFile: false,
            inputSourceMap: false,
          })
        )
        return { code: /** @type {string} */ (ret.code) }
      }
    },
  }
}

export default defineThreadSafePluginImplementation((_options, _context) => {
  return babelPlugin()
})
