import { moduleFederationPlugin } from 'rolldown/experimental'
import { RolldownOutput } from 'rolldown'
import { defineTest } from 'rolldown-tests'
import { getOutputChunkNames } from 'rolldown-tests/utils'
import { expect } from 'vitest'
import path from 'node:path'

export default defineTest({
  config: {
    external: ['node:assert', 'node:fs', '@module-federation/runtime'],
    plugins: [
      moduleFederationPlugin({
        name: 'mf-host',
        remotes: {
          app: {
            name: 'app',
            type: 'module',
            entry:
              'file://' +
              path.join(import.meta.dirname, './dist/remote-entry.js'),
          },
        },
        runtimePlugins: [
          path.join(import.meta.dirname, 'mf-runtime-plugin.js'),
        ],
      }),
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
    // Here avoid starting dev-server to load the remote script.
    // - using module federation runtime `createScriptNode` to load the remote script, but the internal implementation using `fetch`, it is not support `file` protocol url. And also it is using `vm.SourceTextModule` to execute esm, tis feature is only available with the `--experimental-vm-modules` command flag enabled.
    // - Using module federation runtime plugin to load the remote, here setting the `globalThis.remote` and using it at `mf-runtime-plugin.js`.
    // @ts-ignore
    globalThis.remote = remote
    // @ts-ignore
    await import('./dist/main.js')
  },
})
