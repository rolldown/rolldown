import { test } from 'vitest'
import { build } from 'rolldown'
import { moduleFederationPlugin } from 'rolldown/experimental'
import path from 'node:path'
import { describe } from 'node:test'

describe('module-federation', () => {
  test('module-federation', async () => {
    // build host
    await build({
      input: './main.js',
      cwd: import.meta.dirname,
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
          runtimePlugins: [
            path.join(import.meta.dirname, './mf-runtime-plugin.js'),
          ],
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
  })
})
