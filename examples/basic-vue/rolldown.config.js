import { defineConfig } from 'rolldown'
import replace from '@rollup/plugin-replace'

export default defineConfig({
  input: './index.js',
  resolve: {
    // This needs to be explicitly set for now because oxc resolver doesn't
    // assume default exports conditions. Rolldown will ship with a default that
    // aligns with Vite in the future.
    conditionNames: ['import'],
  },
  plugins: [
    replace({
      'process.env.NODE_ENV': JSON.stringify('production'),
    }),
  ],
})
