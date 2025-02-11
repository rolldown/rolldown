import type { OutputChunk as RolldownOutputChunk } from 'rolldown'
import { defineTest } from 'rolldown-tests'
import { expect } from 'vitest'

export default defineTest({
  config: {
    plugins: [
      {
        name: 'loader',
        async transform(code, id) {
          if (id.includes('bar.js')) {
            const resolved = await this.resolve('./foo.js', id)
            const moduleInfo = this.getModuleInfo(resolved!.id)
            moduleInfo!.moduleSideEffects = true

            // mutate sideEffects of bar.js to `false`
            const moduleInfo1 = this.getModuleInfo(id)
            moduleInfo1!.moduleSideEffects = false
          }
          return {
            code,
          }
        },
      },
    ],
  },
  afterTest: (output) => {
    output.output
      .filter(({ type }) => type === 'chunk')
      .forEach((chunk) => {
        let code = (chunk as RolldownOutputChunk).code
        expect(code.includes(`sideeffects`)).toBe(true)
        expect(code.includes(`bar`)).toBe(false)
      })
  },
})
