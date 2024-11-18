import { defineTest } from '@tests'
import { expect } from 'vitest'
import path from 'node:path'

let fooHookCalls = 0

export default defineTest({
  config: {
    plugins: [
      {
        name: 'test-plugin-context',
        async buildStart() {
          try {
            await this.load({
              id: path.join(__dirname, 'foo.js'),
            })
          } catch (e: any) {
            expect(e.message).toMatch(
              'The `PluginContext.load` only work at `resolveId/load/transform/moduleParsed` hooks',
            )
          }
        },
        async load(id) {
          if (id.endsWith('main.js')) {
            const moduleInfo = await this.load({
              id: path.join(__dirname, 'foo.js'),
              moduleSideEffects: false,
            })
            expect(moduleInfo.code!.includes('foo')).toBe(true)
          }
          if (id.endsWith('foo.js')) {
            fooHookCalls++
          }
        },
        async transform(code, id) {
          if (id.endsWith('main.js')) {
            const moduleInfo = await this.load({
              id: path.join(__dirname, 'foo.js'),
            })
            expect(moduleInfo.code!.includes('foo')).toBe(true)
            // should reusing exiting modules
            expect(fooHookCalls).toBe(1)
          }
        },
      },
    ],
  },
  beforeTest: () => {
    fooHookCalls = 0
  },
  afterTest: (output) => {
    expect(output.output[0].code.includes(`console.log`)).toBe(false)
  },
})
