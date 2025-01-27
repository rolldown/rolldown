import { test, expect } from 'vitest'
import { build } from 'rolldown'
import { moduleFederationPlugin } from 'rolldown/experimental'
import path from 'node:path'
import { describe } from 'node:test'

describe('module-federation', () => {
  test('module-federation', async () => {
    // Make sure the host and remote using different @module-federation/runtime

    // build host
    await build({
      input: './main.js',
      cwd: import.meta.dirname,
      external: ['node:assert', '@module-federation/runtime'],
      plugins: [
        moduleFederationPlugin({
          name: 'mf-host',
          remotes: {
            app: {
              name: 'app',
              type: 'module',
              // using file protocol to ensure the entry is a url
              entry:
                'file://' +
                path.join(import.meta.dirname, './dist/remote-entry.js'),
            },
          },
          runtimePlugins: ['./mf-runtime-plugin.js'],
        }),
      ],
    })

    // build remote
    await build({
      input: './expose.js',
      cwd: import.meta.dirname,
      plugins: [
        moduleFederationPlugin({
          name: 'mf-remote',
          filename: 'remote-entry.js',
          exposes: {
            './expose': './expose.js',
          },
        }),
      ],
    })

    // Test the exposed module
    // @ts-ignore
    const expose = await import('./dist/expose.js')
    expect(expose.value).toBe('expose')

    // Test the remote entry
    // @ts-ignore
    const remote = await import('./dist/remote-entry.js')
    const remoteExposeFactory = await remote.get('./expose')
    expect(remoteExposeFactory().value).toBe('expose')
    expect(typeof remote.init).toBe('function')

    // Test host
    // Here avoid starting dev-server to load the remote script.
    // - using module federation runtime `createScriptNode` to load the remote script, but the internal implementation using `fetch`, it is not support `file` protocol url. And also it is using `vm.SourceTextModule` to execute esm, tis feature is only available with the `--experimental-vm-modules` command flag enabled.
    // - Using module federation runtime plugin to load the remote, here setting the `globalThis.remote` and using it at `mf-runtime-plugin.js`.
    // @ts-ignore
    globalThis.remote = remote
    // @ts-ignore
    await import('./dist/main.js')
  })
})
