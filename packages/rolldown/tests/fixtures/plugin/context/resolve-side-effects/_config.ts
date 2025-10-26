import { defineTest } from 'rolldown-tests'
import { expect, vi } from 'vitest'
import { viteResolvePlugin } from 'rolldown/experimental'

const fn = vi.fn()

export default defineTest({
  config: {
    plugins: [
      {
        name: 'test-plugin-context',
        async buildStart(this) {
          const ret = await this.resolve('@rolldown/test-side-effects-field')
          if (!ret) {
            throw new Error('resolve failed')
          }
          expect(ret.moduleSideEffects).toBe(false)
          fn()
        },
      },
      viteResolvePlugin({
        resolveOptions: {
          isBuild: true,
          isProduction: true,
          asSrc: false,
          preferRelative: false,
          root: import.meta.dirname,
          scan: false,
          mainFields: ['main'],
          conditions: [],
          externalConditions: [],
          extensions: ['.js'],
          tryIndex: false,
          preserveSymlinks: false,
          tsconfigPaths: false,
        },
        environmentConsumer: 'client',
        environmentName: 'test',
        builtins: [],
        external: [],
        noExternal: [],
        dedupe: [],
        legacyInconsistentCjsInterop: false,
        resolveSubpathImports() {
          throw new Error('Not implemented')
        }
      })
    ],
  },
  afterTest: () => {
    expect(fn).toHaveBeenCalledTimes(1)
  },
})
