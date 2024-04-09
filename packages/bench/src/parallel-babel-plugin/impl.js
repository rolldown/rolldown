import { defineThreadSafePluginImplementation } from 'rolldown/thread-safe-plugin'
import { transform } from 'sucrase'
import nodePath from 'node:path'

/** @returns {import('rolldown').Plugin} */
export const babelPlugin = () => {
  return {
    name: 'parallel-babel-plugin',
    transform(code, id) {
      const ext = nodePath.extname(id)
      if (ext === '.ts' || ext === '.tsx') {
        /** @type {import('sucrase').Transform[]} */
        const transforms = ['typescript']
        if (ext === '.tsx') {
          transforms.push('jsx')
        }

        const ret = transform(code, { filePath: id, transforms })
        return { code: ret.code }
      }
    },
  }
}

export default defineThreadSafePluginImplementation((_options, _context) => {
  return babelPlugin()
})
