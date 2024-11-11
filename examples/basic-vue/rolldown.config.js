import { defineConfig } from 'rolldown'
import path from 'path'

export default defineConfig({
  input: './index.js',
  resolve: {
    // This needs to be explicitly set for now because oxc resolver doesn't
    // assume default exports conditions. Rolldown will ship with a default that
    // aligns with Vite in the future.
    conditionNames: ['import'],
  },
  plugins: [
    {
      name: 'test-plugin-context',
      async resolveId(id) {
        if (id.endsWith('index.js')) {
          const moduleInfo = await this.load({ id: path.join(import.meta.dirname, 'lib.js')})
          console.log(moduleInfo)
          }
      },
    },
  ],
})
