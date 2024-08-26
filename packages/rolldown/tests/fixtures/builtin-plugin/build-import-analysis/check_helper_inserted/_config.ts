import { buildImportAnalysisPlugin } from 'rolldown/experimental'
import { defineTest } from '@tests'
import { expect } from 'vitest'

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
  },
  async afterTest(output) {
    await import('./assert.mjs')
    output.output.forEach((item) => {
      if (item.type === 'chunk') {
        Object.keys(item.modules).forEach((key) => {
          expect(key).not.contains('vite')
        })
      }
    })
  },
})
