import { buildImportAnalysisPlugin } from 'rolldown/experimental'
import { defineTest } from 'rolldown-tests'
import { expect, vi } from 'vitest'

const onLogFn = vi.fn()

export default defineTest({
  skipComposingJsPlugin: true,
  config: {
    input: './main.js',
    plugins: [
      {
        // insert some dummy runtime flag to assert the runtime behavior
        name: 'insert_dummy_flag',
        transform(code, id) {
          let runtimeCode = `
const __VITE_IS_MODERN__ = false;

`
          return {
            code: runtimeCode + code,
          }
        },
      },
      buildImportAnalysisPlugin({
        preloadCode: `
export const __vitePreload = (v) => {
  return v()
};
`,
        insertPreload: true,
        optimizeModulePreloadRelativePaths: false,
        renderBuiltUrl: false,
        isRelativeBase: false,
      }),
    ],
    onLog(level, log) {
      expect(level).toBe('warn')
      expect(log.code).toBe('UNRESOLVED_IMPORT')
      expect(log.message).toContain(
        "Could not resolve 'node:assert' in main.js",
      )
      onLogFn()
    },
  },
  async afterTest(output) {
    expect(onLogFn).toHaveBeenCalledTimes(1)

    await import('./assert.mjs')

    output.output.forEach((item) => {
      if (item.type === 'chunk' && item.name === 'main') {
        expect(item.code).to.not.includes('import.meta.url')
      }
    })
  },
})
