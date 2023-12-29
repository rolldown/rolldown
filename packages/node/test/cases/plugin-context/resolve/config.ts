import type { RollupOptions } from '@rolldown/node'
import { PluginContext } from 'rollup'
import { expect } from 'vitest'
import path from 'path'

const config: RollupOptions = {
  plugins: [
    {
      name: 'test-plugin-context',
      buildStart: async function (this: PluginContext) {
        const value = await this.resolve(
          './main.js',
          path.join(__dirname, './main.js'),
        )
        expect(value!.id).toBe(path.join(__dirname, './main.js'))
      },
    },
  ],
}

export default {
  config,
}
