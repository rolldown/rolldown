import { expect } from 'vitest'
import { defineTest } from 'rolldown-tests'
import { Plugin } from 'rolldown-tests/types'

const plugins: Plugin[] = []
const hooks = [
  'augmentChunkHash',
  'buildEnd',
  'buildStart',
  'generateBundle',
  'writeBundle',
  'load',
  'moduleParsed',
  'options',
  'outputOptions',
  // 'renderDynamicImport',
  // 'renderError'
  'renderChunk',
  'renderStart',
  'resolveDynamicImport',
  // 'resolveFileUrl',
  'resolveId',
  // 'resolveImportMeta',
  // 'shouldTransformCachedModule',
  'transform',
  'banner',
  'footer',
  'intro',
  'outro',
  'onLog',
]

const calledHooks: Record<string, string[]> = {}
for (const hook of hooks) {
  calledHooks[hook] = []
}

addPlugin(null)
addPlugin('pre')
addPlugin('post')
addPlugin('post')
addPlugin('pre')
addPlugin()
function addPlugin(order?: 'pre' | 'post' | null) {
  const name = `${order}-${plugins.length}`
  const plugin = { name } as Plugin
  for (const hook of hooks) {
    // @ts-expect-error hook is keyof Plugin
    plugin[hook] = {
      order,
      handler() {
        if (!calledHooks[hook].includes(name)) {
          calledHooks[hook].push(name)
        }
      },
    }
  }
  plugins.push(plugin)
}

export default defineTest({
  config: {
    plugins: [
      ...plugins,
      {
        name: 'add-log',
        buildStart() {
          this.warn('a warning')
        },
      },
    ],
  },
  afterTest: () => {
    for (const hook of hooks) {
      expect(calledHooks[hook]).toStrictEqual([
        'pre-1',
        'pre-4',
        'null-0',
        'undefined-5',
        'post-2',
        'post-3',
      ])
    }
  },
})
