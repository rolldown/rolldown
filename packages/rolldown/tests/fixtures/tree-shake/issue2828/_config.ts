import { defineTest } from 'rolldown-tests'
import { expect } from 'vitest'

const useLoadHook = true

export default defineTest({
  config: {
    treeshake: {
      moduleSideEffects: false,
    },
    plugins: [
      {
        name: 'loader',
        resolveId(id) {
          if (id === 'foo') {
            return { id, moduleSideEffects: useLoadHook ? undefined : true }
          }
        },
        load(id) {
          if (id === 'foo') {
            return {
              code: 'console.log("foo")',
              moduleSideEffects: useLoadHook ? true : undefined,
            }
          }
        },
      },
    ],
  },
  afterTest: (output) => {
    expect(output.output[0].code).toContain('console.log("foo")')
  },
})
