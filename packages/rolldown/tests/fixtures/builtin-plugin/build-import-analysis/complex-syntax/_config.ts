import { expect } from 'vitest'
import { defineTest } from 'rolldown-tests'
import { buildImportAnalysisPlugin } from 'rolldown/experimental'

export default defineTest({
  config: {
    input: './main.js',
    plugins: [
      {
        // insert some dummy runtime flag to assert the runtime behavior
        name: 'insert_dummy_flag',
        transform(code) {
          let runtimeCode = `const __VITE_IS_MODERN__ = false;`
          return {
            code: runtimeCode + code,
          }
        },
      },
      buildImportAnalysisPlugin({
        preloadCode: `export const __vitePreload = (v) => { return v() };`,
        insertPreload: true,
        optimizeModulePreloadRelativePaths: false,
        renderBuiltUrl: true,
        isRelativeBase: false,
      }),
    ],
  },
  async afterTest(output) {
    await import('./assert.mjs')
    output.output.forEach((item) => {
      if (item.type === 'chunk' && item.name === 'main') {
        expect(item.code).to.includes('import.meta.url')
      }
    })
  },
})
