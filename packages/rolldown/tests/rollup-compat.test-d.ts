import { test, expectTypeOf, describe } from 'vitest'
import type { Plugin as RollupPlugin } from 'rollup'
import { defineConfig, type InputOptions, type OutputOptions, type Plugin as RolldownRawPlugin } from 'rolldown'

describe('plugin type compatibility', () => {
  type PluginsOption = InputOptions['plugins']
  type OutputPluginsOption = OutputOptions['plugins']

  test('can assign rollup plugins to `plugins` option', () => {
    const rollupPluginInstance = undefined as unknown as RollupPlugin
    expectTypeOf(rollupPluginInstance).toExtend<PluginsOption>()

    // this should not error
    defineConfig({
      plugins: [rollupPluginInstance]
    })
  })

  test('can assign rollup plugins to `output.plugins` option', () => {
    const rollupPluginInstance = undefined as unknown as RollupPlugin
    expectTypeOf(rollupPluginInstance).toExtend<OutputPluginsOption>()

    // this should not error
    defineConfig({
      output: {
        plugins: [rollupPluginInstance]
      }
    })
  })

  test('input suggestions for hooks works', () => {
    const plugin: PluginsOption = {
      name: 'test',

      // ^ input suggestion should work here
    }
    expectTypeOf(plugin).toEqualTypeOf<RolldownRawPlugin | { name: string }>()

    const buildS = undefined
    defineConfig({
      plugins: [
        {
          name: 'test',
          // @ts-expect-error -- only known properties should be allowed
          buildS
          //    ^ input suggestion should work here
        }
      ]
    })
  })
})
