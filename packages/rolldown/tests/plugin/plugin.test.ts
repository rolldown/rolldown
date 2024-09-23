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
  } catch {
    // Here `renderError` test will crash it, here avoid bubble it.
    // console.log(error)
  }
}

test('Plugin renderError hook', async () => {
  const renderErrorFn = vi.fn()
  const renderChunkFn = vi.fn()
  await buildWithPlugin({
    renderChunk() {
      renderChunkFn()
      throw new Error('renderChunk error')
    },
    renderError: (error) => {
      renderErrorFn()
      expect(renderChunkFn).toHaveBeenCalledTimes(1)
      expect(error).toBeInstanceOf(Error)
    },
  })
  expect(renderErrorFn).toHaveBeenCalledTimes(1)
})

describe('Plugin buildEnd hook', async () => {
  test('call buildEnd hook with error', async () => {
    const buildEndFn = vi.fn()
    await buildWithPlugin({
      load() {
        throw new Error('load error')
      },
      buildEnd: (error) => {
        buildEndFn()
        expect(error).toBeInstanceOf(Error)
      },
    })
    expect(buildEndFn).toHaveBeenCalledTimes(1)
  })

  test('call buildEnd hook without error', async () => {
    const buildEndFn = vi.fn()
    await buildWithPlugin({
      buildEnd: (error) => {
        buildEndFn()
        expect(error).toBeNull()
      },
    })
    expect(buildEndFn).toHaveBeenCalledTimes(1)
  })
})

describe('Plugin closeBundle hook', async () => {
  test('call closeBundle hook if has error', async () => {
    const closeBundleFn = vi.fn()
    await buildWithPlugin({
      load() {
        throw new Error('load error')
      },
      closeBundle: () => {
        closeBundleFn()
      },
    })
    expect(closeBundleFn).toHaveBeenCalledTimes(1)
  })

  test('call closeBundle with bundle close', async () => {
    const closeBundleFn = vi.fn()
    const build = await rolldown({
      input: './main.js',
      cwd: import.meta.dirname,
      plugins: [
        {
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
