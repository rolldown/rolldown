// @ts-check
import { defineConfig } from 'rolldown'

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
      name: 'test',
      transform: {
        // filter: {
        //   code: {
        //     include: ['test']
        //   },
        // },
        handler() {},
      },
      load: {
        handler(id) {
          console.log(`id: `, id)
          return null
        },
        filter: {
          id: {
            include: ['dir/**/*.res'],
            exclude: [ "dir/**/*.cs"],
          },
        },
      },
      banner: {
        handler() {
          return ''
        },
      },
    },
  ],
})
