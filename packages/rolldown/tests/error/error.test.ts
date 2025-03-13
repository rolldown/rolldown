import { expect, test, vi, describe } from 'vitest'
import { rolldown, Plugin } from 'rolldown'

async function buildWithPlugin(plugin: Plugin) {
  try {
    const build = await rolldown({
      input: './main.js',
      cwd: import.meta.dirname,
      plugins: [plugin],
    })
    await build.write({})
  } catch (e) {
    return e as Error
  }
}

test('Plugin renderError hook', async () => {
  const renderErrorFn = vi.fn()
  const renderChunkFn = vi.fn()
  const error = await buildWithPlugin({
    name: 'test',
    renderStart() {
      renderChunkFn()
      throw new Error('renderStart error')
    },
    renderError: (error) => {
      renderErrorFn()
      expect(error!.message).toContain('renderStart error')
    },
  })
  expect(error!.message).toContain('renderStart error')
  expect(renderErrorFn).toHaveBeenCalledTimes(1)
})

describe('Plugin buildEnd hook', async () => {
  test('call buildEnd hook with error', async () => {
    const buildEndFn = vi.fn()
    const error = await buildWithPlugin({
      name: 'test',
      buildStart() {
        throw new Error('buildStart error')
      },
      buildEnd: (error) => {
        buildEndFn()
        expect(error!.message).toContain('buildStart error')
      },
    })
    expect(error!.message).toContain('buildStart error')
    expect(buildEndFn).toHaveBeenCalledTimes(1)
  })

  test('call buildEnd hook without error', async () => {
    const buildEndFn = vi.fn()
    const error = await buildWithPlugin({
      name: 'test',
      buildEnd: (error) => {
        buildEndFn()
        expect(error).toBeUndefined()
      },
    })
    expect(error).toBeUndefined()
    expect(buildEndFn).toHaveBeenCalledTimes(1)
  })
})

describe('Plugin closeBundle hook', async () => {
  test('call closeBundle hook if has error', async () => {
    const closeBundleFn = vi.fn()
    const error = await buildWithPlugin({
      name: 'test',
      load() {
        throw new Error('load error')
      },
      closeBundle: () => {
        closeBundleFn()
      },
    })
    expect(error!.message).toContain('load error')
    expect(closeBundleFn).toHaveBeenCalledTimes(1)
  })

  test('call closeBundle with bundle close', async () => {
    const closeBundleFn = vi.fn()
    const build = await rolldown({
      input: './main.js',
      cwd: import.meta.dirname,
      plugins: [
        {
          name: 'test',
          closeBundle: () => {
            closeBundleFn()
          },
        },
      ],
    })
    await build.close()
    expect(closeBundleFn).toHaveBeenCalledTimes(1)
  })

  test('should error at generate if bundle already closed', async () => {
    try {
      const build = await rolldown({
        input: './main.js',
        cwd: import.meta.dirname,
      })
      await build.close()
      await build.write()
    } catch (error: any) {
      expect(error.message).toMatch(
        `Rolldown internal error: Bundle is already closed, no more calls to 'generate' or 'write' are allowed.`,
      )
    }
  })
})

test('call transformContext error', async () => {
  const error = await buildWithPlugin({
    name: 'test',
    transform() {
      this.error('transform hook error')
    },
  })
  expect(error!.message).toContain('transform hook error')
})

describe('Error output format', () => {
  function cleanStack(stack: string) {
    return stack
      .split('\n')
      .map((line) => {
        return line.replace(
          /at (.*?) \((?:(.*[\/\\])?([^\/\\]+):)?(\d+:\d+)\)/,
          'at $1 ($3:$4)',
        )
      })
      .join('\n')
  }

  test('should correctly output the custom error defined on the js side', async () => {
    try {
      const build = await rolldown({
        input: './main.js',
        cwd: import.meta.dirname,
        plugins: [
          {
            name: 'test-plugin',
            buildEnd() {
              throw new Error('js side error')
            },
          },
        ],
      })
      await build.write()
    } catch (error: any) {
      expect(cleanStack(error.message)).toMatchSnapshot()
    }
  })

  test('should correctly output the custom error defined on the rust side', async () => {
    try {
      const build = await rolldown({
        input: './error.js',
        cwd: import.meta.dirname,
      })
      await build.write()
    } catch (error: any) {
      const message = error.message.replace(
        /at (.*?) \((?:(.*[\/\\])?([^\/\\]+):)?(\d+:\d+)\)/,
        'at $1 ($3:$4)',
      )
      expect(message).toMatchSnapshot()
    }
  })
})
