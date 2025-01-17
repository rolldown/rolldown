import { moduleFederationPlugin } from 'rolldown/experimental'
import { RolldownOutput } from 'rolldown'
import { defineTest } from '@tests'
import { getOutputChunkNames } from '@tests/utils'
import { expect } from 'vitest'
import path from 'node:path'

export default defineTest({
  config: {
    external: ['node:assert', '@module-federation/runtime'],
    plugins: [
      moduleFederationPlugin({
        name: 'mf-remote',
        filename: 'remote-entry.js',
        exposes: {
          './expose': './expose.js',
        },
        remotes: {
          app: {
            name: 'app',
            type: 'module',
            entry: path.join(import.meta.dirname, './dist/remote-entry.js'),
          },
        },
      }),
    ],
    output: {
      chunkFileNames: '[name].js',
    },
  },
  async afterTest(output: RolldownOutput) {
    const chunksNames = getOutputChunkNames(output)
    expect(chunksNames.includes('remote-entry.js')).toBe(true)
    expect(chunksNames.includes('expose.js')).toBe(true)

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

    // Test host
    // @ts-ignore
    // await import('./dist/main.js')
  },
})
