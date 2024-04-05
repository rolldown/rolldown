import { expect, describe, test, vi, afterEach } from 'vitest'
import { CommandDef } from 'citty'

vi.mock('consola/utils', async (importOriginal) => {
  const mod = await importOriginal<typeof import('consola/utils')>()
  return {
    ...mod,
    colors: {
      cyan: (str: string) => str,
      underline: (str: string) => str,
      bold: (str: string) => str,
    },
  }
})

afterEach(() => {
  vi.resetModules()
})

describe('renderUsage', () => {
  test('render', async () => {
    vi.doMock('../../src/cli/env.js', () => ({
      isColorSupported: false,
    }))

    const { renderUsage } = await import('../../src/cli/usage.js')
    const cmd = {
      meta: {
        name: 'rolldown',
        version: '0.10.1',
        description:
          'Fast JavaScript/TypeScript bundler in Rust with Rollup-compatible API.',
      },
      args: {
        config: {
          type: 'string',
          alias: 'c',
          description:
            'Use this config file (if argument is used but value is unspecified, defaults to rolldown.config.js)',
        },
        help: {
          type: 'boolean',
          alias: 'h',
          description: 'Show this help message',
        },
      },
    } satisfies CommandDef

    expect(await renderUsage(cmd)).toMatchSnapshot()
  })
})
