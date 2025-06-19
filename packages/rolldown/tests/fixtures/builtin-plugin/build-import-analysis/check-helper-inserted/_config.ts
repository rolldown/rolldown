import { defineTest } from 'rolldown-tests'
import { buildImportAnalysisPlugin } from 'rolldown/experimental'

export default defineTest({
  skipComposingJsPlugin: true,
  config: {
    input: './main.js',
    plugins: [
      {
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
        renderBuiltUrl: false,
        isRelativeBase: false,
      }),
    ],
  },
  async afterTest() {
    await import('./assert.mjs')
  },
})
