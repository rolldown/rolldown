import { moduleFederationPlugin } from 'rolldown/experimental'
import { RolldownOutput } from 'rolldown'
import { defineTest } from '@tests'
import { getOutputChunkNames } from '@tests/utils'
import { expect } from 'vitest'

export default defineTest({
  config: {
    external: ['@module-federation/enhanced'],
    plugins: [
      moduleFederationPlugin({
        name: 'mf-remote',
        filename: 'remote-entry.js',
        exposes: {
          './expose': './expose.js',
        },
      }),
    ],
    output: {
      chunkFileNames: '[name].js',
    },
  },
  async afterTest(output: RolldownOutput) {
    expect(getOutputChunkNames(output)).toMatchInlineSnapshot(`
      [
        "main.js",
        "remote-entry.js",
        "expose.js",
      ]
    `)
    // Test the exposed module
    // @ts-ignore
    const expose = await import('./dist/expose.js')
    expect(expose.value).toBe('expose')

    // Test the remote entry
    // @ts-ignore
    const remote = await import('./dist/remote-entry.js')
    const remoteExpose = await remote.get('./expose')
    expect(remoteExpose.value).toBe('expose')
    expect(typeof remote.init).toBe('function')
  },
})
