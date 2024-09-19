import { defineConfig } from 'rolldown'
import { aliasPlugin } from 'rolldown/experimental'

export default defineConfig({
  input: {
    entry: './src/index.ts',
  },
  resolve: {
    // alias: {
    //   '@': 'src'
    // }
  },
  plugins: [
    aliasPlugin({
      entries: [
        {
          find: '@',
          replacement: 'src',
        },
      ],
    }),
  ],
  // resolve: {
  //   // This needs to be explicitly set for now because oxc resolver doesn't
  //   // assume default exports conditions. Rolldown will ship with a default that
  //   // aligns with Vite in the future.
  //   conditionNames: ['import'],
  // },
})
