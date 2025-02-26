import { test, expect, describe, vi } from 'vitest'
import { build } from 'rolldown'
import { moduleFederationPlugin } from 'rolldown/experimental'
import path from 'node:path'

describe('module-federation', () => {
  test('module-federation', async () => {
    // Make sure the host and remote using different @module-federation/runtime

    // build host
    await build({
      input: './host-entry.js',
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
          shared: {
            'test-shared': {
              singleton: true,
            },
            'test-shared-cjs': {
              singleton: true,
            },
          },
          runtimePlugins: ['./mf-runtime-plugin.js'],
        }),
      ],
      output: {
        dir: 'dist/host',
      },
    })

    const onLogFn = vi.fn()

    // build remote
    await build({
      input: './remote-expose.js',
      cwd: import.meta.dirname,
      plugins: [
        {
          name: 'mock-vite-resolver-plugin',
          resolveId(id) {
            if (id === 'test-shared-cjs') {
              return require.resolve('test-shared-cjs')
            }
          },
        },
        moduleFederationPlugin({
          name: 'mf-remote',
          filename: 'remote-entry.js',
          exposes: {
            './expose': './remote-expose.js',
            './expose-foo': './remote-foo.js',
          },
          shared: {
            'test-shared': {
              singleton: true,
            },
            'test-shared-cjs': {
              singleton: true,
            },
          },
          manifest: true,
          getPublicPath: 'file://public_path',
        }),
      ],
      output: {
        dir: 'dist/remote',
        chunkFileNames: '[name].js',
      },
      onLog(level, log) {
        expect(level).toBe('warn')
        expect(log.code).toBe('EVAL')
        onLogFn()
      },
    })

    expect(onLogFn).toHaveBeenCalledTimes(1)

    // Test the remote manifest json
    // @ts-ignore
    const remoteManifest = await import('./dist/remote/mf-manifest.json')
    expect(remoteManifest.default).toMatchSnapshot()

    // Test the remote entry
    // @ts-ignore
    const remote = await import('./dist/remote/remote-entry.js')
    expect(typeof remote.get).toBe('function')
    expect(typeof remote.init).toBe('function')

    // Test host
    // Here avoid starting dev-server to load the remote script.
    // - using module federation runtime `createScriptNode` to load the remote script, but the internal implementation using `fetch`, it is not support `file` protocol url. And also it is using `vm.SourceTextModule` to execute esm, tis feature is only available with the `--experimental-vm-modules` command flag enabled.
    // - Using module federation runtime plugin to load the remote, here setting the `globalThis.remote` and using it at `mf-runtime-plugin.js`.
    // @ts-ignore
    globalThis.remote = remote
    // @ts-ignore
    globalThis.mfShared = 0
    // @ts-ignore
    await import('./dist/host/host-entry.js')
    // Make sure only one instance of shared module is loaded
    // @ts-ignore
    expect(globalThis.mfShared).toBe(1)
  })
})
