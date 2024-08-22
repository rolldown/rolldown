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
      name: "test",
      transform: {
        filter: {
          code: {
          }
        },
        handler() {}
      },
      resolveId: {
        handler() {},
        filter: {
          id: {
            include: ["test"],
            exclude: ["test", /test/]
          },
        }
      },
      banner: {
        handler() {return ''}
      }
    }
  ]
})
