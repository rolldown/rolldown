import { expect, test, vi } from 'vitest'
import { watch, RolldownWatcher } from 'rolldown'
import fs from 'node:fs'
import path from 'node:path'
import { sleep, waitUtil } from 'rolldown-tests/utils'

test.sequential('watch', async () => {
  const { input, output } = await createTestInputAndOutput('watch')
  const watchChangeFn = vi.fn()
  const closeWatcherFn = vi.fn()
  const watcher = watch({
    input,
    output: { file: output },
    plugins: [
      {
        name: 'test watchChange',
        watchChange(id, event) {
          // The macos emit create event when the file is changed, not sure the reason,
          // so here only check the update event
          if (event.event === 'update') {
            watchChangeFn()
            expect(id).toBe(input)
          }
        },
      },
      {
        name: 'test closeWatcher',
        closeWatcher() {
          closeWatcherFn()
        },
      },
    ],
  })
  // should run build once
  await waitBuildFinished(watcher)

  // edit file
  fs.writeFileSync(input, 'console.log(2)')
  await waitUtil(() => {
    expect(fs.readFileSync(output, 'utf-8').includes('console.log(2)')).toBe(
      true,
    )
    // The different platform maybe emit multiple events
    expect(watchChangeFn).toBeCalled()
  })

  await watcher.close()
  expect(closeWatcherFn).toBeCalledTimes(1)
})

test.sequential('watch files after scan stage', async () => {
  const { input, output } = await createTestInputAndOutput(
    'watch-files-after-scan',
  )
  const watcher = watch({
    input,
    output: { file: output },
    plugins: [
      {
        name: 'test',
        renderStart() {
          fs.writeFileSync(input, 'console.log(2)')
        },
      },
    ],
  })
  // should run build once
  await waitBuildFinished(watcher)

  await waitUtil(() => {
    expect(fs.readFileSync(output, 'utf-8').includes('console.log(2)')).toBe(
      true,
    )
  })

  await watcher.close()
})

test.sequential('watch close', async () => {
  const { input, output } = await createTestInputAndOutput('watch-close')
  const watcher = watch({
    input,
    output: { file: output },
  })
  await waitBuildFinished(watcher)

  await watcher.close()
  // edit file
  fs.writeFileSync(input, 'console.log(3)')
  await waitUtil(() => {
    // The watcher is closed, so the output file should not be updated
    expect(fs.readFileSync(output, 'utf-8').includes('console.log(1)')).toBe(
      true,
    )
  })
})

test.sequential('watch event', async () => {
  const { input, outputDir } = await createTestInputAndOutput('watch-event')
  const watcher = watch({
    input,
    output: { dir: outputDir },
    watch: {
      buildDelay: 50,
    },
  })

  const events: any[] = []
  watcher.on('event', (event) => {
    if (event.code === 'BUNDLE_END') {
      expect(event.output).toEqual([outputDir])
      expect(event.duration).toBeTypeOf('number')
      events.push({ code: 'BUNDLE_END' })
    } else {
      events.push(event)
    }
  })
  const restartFn = vi.fn()
  watcher.on('restart', restartFn)
  const closeFn = vi.fn()
  watcher.on('close', closeFn)
  const changeFn = vi.fn()
  watcher.on('change', (id, event) => {
    // The macos emit create event when the file is changed, not sure the reason,
    // so here only check the update event
    if (event.event === 'update') {
      changeFn()
      expect(id).toBe(input)
    }
  })

  await waitUtil(() => {
    // test first build event
    expect(events).toEqual([
      { code: 'START' },
      { code: 'BUNDLE_START' },
      { code: 'BUNDLE_END' },
      { code: 'END' },
    ])
  })

  // edit file
  events.length = 0
  fs.writeFileSync(input, 'console.log(3)')
  await waitUtil(() => {
    // Note: The different platform maybe emit multiple events
    expect(events).toEqual([
      { code: 'START' },
      { code: 'BUNDLE_START' },
      { code: 'BUNDLE_END' },
      { code: 'END' },
    ])
    expect(restartFn).toBeCalled()
    expect(changeFn).toBeCalled()
  })

  await watcher.close()
  // the listener is called with async
  await waitUtil(() => {
    expect(closeFn).toBeCalled()
  })
})

test.sequential('watch event off', async () => {
  const { input, outputDir } = await createTestInputAndOutput('watch-event-off')
  const watcher = watch({
    input,
    output: { dir: outputDir },
    watch: {
      buildDelay: 50,
    },
  })
  const eventFn = vi.fn()
  watcher.on('event', eventFn)
  await waitBuildFinished(watcher)
  expect(eventFn).toHaveBeenCalled()

  eventFn.mockClear()
  watcher.off('event', eventFn);

  fs.writeFileSync(input, 'console.log(12)')
  await waitBuildFinished(watcher)
  expect(eventFn).not.toHaveBeenCalled()

  await watcher.close()
})

test.sequential('watch BUNDLE_END event result.close() + closeBundle', async () => {
  const { input, outputDir } = await createTestInputAndOutput('watch-event-close-closeBundle')
  const closeBundleFn = vi.fn()
  const watcher = watch({
    input,
    output: { dir: outputDir },
    plugins: [
      {
        name: 'test',
        closeBundle: closeBundleFn
      }
    ]
  })
  watcher.on('event', async (event) => {
    if (event.code === 'BUNDLE_END') {
      await event.result.close()
    }
  })
  await waitBuildFinished(watcher)

  expect(closeBundleFn).toBeCalledTimes(1)

  // The `result.close` could be call multiply times.
  fs.writeFileSync(input, 'console.log(3)')
  await waitBuildFinished(watcher)
  expect(closeBundleFn).toBeCalledTimes(2)

  await watcher.close()
})

test.sequential('watch ERROR event result.close() + closeBundle', async () => {
  const { input, outputDir } = await createTestInputAndOutput('watch-event-ERROR-close-closeBundle')
  const closeBundleFn = vi.fn()
  const watcher = watch({
    input,
    output: { dir: outputDir },
    plugins: [
      {
        name: 'test',
        buildStart() {
          throw new Error('test error')
        },
        closeBundle: closeBundleFn
      }
    ]
  })
  watcher.on('event', async (event) => {
    if (event.code === 'ERROR') {
      await event.result.close()
    }
  })

  await waitUtil(() => {
    expect(closeBundleFn).toBeCalledTimes(2) // build error call once + result.close() call once
  })

  await watcher.close()
})

test.sequential('watch BUNDLE_END event output + "file" option', async () => {
  const { input, output } = await createTestInputAndOutput('watch-event')
  const watcher = watch({
    input,
    output: { file: output },
  })

  const eventFn = vi.fn()
  watcher.on('event', (event) => {
    if (event.code === 'BUNDLE_END') {
      eventFn()
      expect(event.output).toEqual([output])
    }
  })

  await waitUtil(() => {
    // test first build event
    expect(eventFn).toBeCalled()
  })

  await watcher.close()
})

test.sequential('watch event avoid deadlock #2806', async () => {
  const { input, output } = await createTestInputAndOutput(
    'watch-event-avoid-dead-lock',
  )
  const watcher = watch({
    input,
    output: { file: output },
  })

  const testFn = vi.fn()
  let listening = false
  watcher.on('event', (event) => {
    if (event.code === 'BUNDLE_END' && !listening) {
      listening = true
      // shouldn't deadlock
      watcher.on('event', () => {
        if (event.code === 'BUNDLE_END') {
          testFn()
        }
      })
    }
  })

  await waitBuildFinished(watcher)

  fs.writeFileSync(input, 'console.log(2)')
  await waitUtil(() => {
    expect(testFn).toBeCalled()
  })

  await watcher.close()
})

test.sequential('watch skipWrite', async () => {
  const { input, output } = await createTestInputAndOutput('watch-skipWrite')
  const watcher = watch({
    input,
    output: { file: output },
    watch: {
      skipWrite: true,
    },
  })
  await waitBuildFinished(watcher)

  expect(fs.existsSync(output)).toBe(false)
  await watcher.close()
})

test.sequential('watch buildDelay', async () => {
  const { input, output } = await createTestInputAndOutput('watch-buildDelay')
  const watcher = watch({
    input,
    output: { file: output },
    watch: {
      buildDelay: 50,
    },
  })
  await waitBuildFinished(watcher)

  const restartFn = vi.fn()
  watcher.on('restart', restartFn)

  fs.writeFileSync(input, 'console.log(4)')
  await sleep(20)
  fs.writeFileSync(input, 'console.log(5)')

  // sleep 200ms to wait the build finished, if the buildDelay is working, the restartFn should be called once
  await sleep(200)
  await waitUtil(() => {
    expect(fs.readFileSync(output, 'utf-8').includes('console.log(5)')).toBe(
      true,
    )
    expect(restartFn).toBeCalledTimes(1)
  })

  await watcher.close()
})

test.sequential('PluginContext addWatchFile', async () => {
  const { input, output } = await createTestInputAndOutput('addWatchFile')
  const { input: foo } = await createTestInputAndOutput('addWatchFile-foo')
  const watcher = watch({
    input,
    output: { file: output },
    plugins: [
      {
        name: 'test',
        buildStart() {
          this.addWatchFile(foo)
        },
      },
    ],
  })

  await waitBuildFinished(watcher)

  const changeFn = vi.fn()
  watcher.on('change', (id, event) => {
    // The macos emit create event when the file is changed, not sure the reason,
    // so here only check the update event
    if (event.event === 'update') {
      changeFn()
      expect(id).toBe(foo)
    }
  })

  // edit file
  fs.writeFileSync(foo, 'console.log(2)\n')
  await waitUtil(() => {
    expect(changeFn).toBeCalled()
  })

  await watcher.close()
})

test.sequential('watch include/exclude', async () => {
  const { input, output } = await createTestInputAndOutput('include-exclude')
  const watcher = watch({
    input,
    output: { file: output },
    watch: {
      exclude: 'main.js',
    },
  })

  await waitBuildFinished(watcher)

  // edit file
  fs.writeFileSync(input, 'console.log(2)')
  await waitUtil(() => {
    // The input is excluded, so the output file should not be updated
    expect(fs.readFileSync(output, 'utf-8').includes('console.log(1)')).toBe(
      true,
    )
  })

  await watcher.close()
})

test.sequential('error handling', async () => {
  // first build error, the watching could be work with recover error
  const { input, output } = await createTestInputAndOutput(
    'error-handling',
    'conso le.log(1)',
  )

  const watcher = watch({
    input,
    output: { file: output },
  })
  const errors: string[] = []
  watcher.on('event', (event) => {
    if (event.code === 'ERROR') {
      errors.push(event.error.message)
    }
  })
  await waitUtil(() => {
    // First build should error
    expect(errors.length).toBe(1)
    expect(errors[0].includes('PARSE_ERROR')).toBe(true)
  })

  fs.writeFileSync(input, 'console.log(2)')
  await waitBuildFinished(watcher)

  // failed again
  fs.writeFileSync(input, 'conso le.log(1)')
  await waitUtil(() => {
    // The different platform maybe emit multiple events
    expect(errors.length > 0).toBe(true)
    expect(errors[0].includes('PARSE_ERROR')).toBe(true)
  })

  // It should be working if the changes are fixed error
  fs.writeFileSync(input, 'console.log(3)')
  await waitUtil(() => {
    expect(fs.readFileSync(output, 'utf-8').includes('console.log(3)')).toBe(
      true,
    )
  })

  await watcher.close()
})

test.sequential('error handling + plugin error', async () => {
  const { input, output } = await createTestInputAndOutput(
    'error-handling-plugin-error',
  )
  const watcher = watch({
    input,
    output: { file: output },
    plugins: [
      {
        name: 'test',
        transform() {
          this.error('plugin error')
        },
      },
    ],
  })
  const errors: string[] = []
  watcher.on('event', (event) => {
    if (event.code === 'ERROR') {
      errors.push(event.error.message)
    }
  })
  await waitUtil(() => {
    // First build should error
    expect(errors.length).toBe(1) // the revert change maybe emit the change event caused it failed
    expect(errors[0].includes('plugin error')).toBe(true)
  })

  errors.length = 0
  fs.writeFileSync(input, 'console.log(2)')
  await waitUtil(() => {
    // The different platform maybe emit multiple events
    expect(errors.length > 0).toBe(true)
    expect(errors[0].includes('plugin error')).toBe(true)
  })

  await watcher.close()
})

test.sequential('watch multiply options', async () => {
  const { input, output, outputDir } = await createTestInputAndOutput(
    'watch-multiply-options',
  )
  const { input: foo, outputDir: fooOutputDir } =
    await createTestInputAndOutput('watch-multiply-options-foo')
  const watcher = watch([
    {
      input,
      output: { dir: outputDir },
    },
    {
      input: foo,
      output: { dir: fooOutputDir },
    },
  ])

  const events: string[] = []
  watcher.on('event', (event) => {
    if (event.code === 'BUNDLE_END') {
      events.push(event.output[0])
    }
  })

  // here should using waitBuildFinished to wait the build finished, because the `input` could be finished before `foo`
  // await waitBuildFinished(watcher)
  await waitUtil(() => {
    expect(fs.readFileSync(output, 'utf-8').includes('console.log(1)')).toBe(
      true,
    )
  })

  fs.writeFileSync(input, 'console.log(2)')
  await waitUtil(() => {
    expect(fs.readFileSync(output, 'utf-8').includes('console.log(2)')).toBe(
      true,
    )
    // Only the input corresponding bundler is rebuild
    expect(events[0]).toEqual(outputDir)
  })

  await watcher.close()
})

test.sequential('warning for multiply notify options', async () => {
  const { input, output } = await createTestInputAndOutput(
    'watch-multiply-options-warning',
  )
  const { input: foo } = await createTestInputAndOutput(
    'watch-multiply-options-warning-foo',
  )
  const onLogFn = vi.fn()
  const watcher = watch([
    {
      input,
      output: { file: output },
      watch: {
        notify: {
          compareContents: false,
        },
      },
    },
    {
      input: foo,
      output: { file: output },
      watch: {
        notify: {
          compareContents: true,
        },
      },
      plugins: [
        {
          name: 'test',
          onLog: (level, log) => {
            onLogFn()
            expect(level).toBe('warn')
            expect(log.code).toBe('MULTIPLY_NOTIFY_OPTION')
          },
        },
      ],
    },
  ])

  await waitUtil(() => {
    expect(onLogFn).toBeCalled()
  })

  await watcher.close()
})

test.sequential('watch close immediately', async () => {
  const { input, output } = await createTestInputAndOutput(
    'watch-close-immediately',
  )
  const watcher = watch({
    input,
    output: { file: output },
  })

  await watcher.close()
})

async function createTestInputAndOutput(dirname: string, content?: string) {
  const dir = path.join(import.meta.dirname, 'temp', dirname)
  fs.mkdirSync(dir, { recursive: true })
  const input = path.join(dir, './main.js')
  fs.writeFileSync(input, content || 'console.log(1)')
  await sleep(60) // TODO: find a way to avoid emit the change event at next test
  const outputDir = path.join(dir, './dist')
  const output = path.join(outputDir, 'main.js')
  return { input, output, dir, outputDir }
}

async function waitBuildFinished(
  watcher: RolldownWatcher,
  updateFn?: () => void,
) {
  return new Promise<void>((resolve) => {
    let listening = false
    watcher.on('event', (event) => {
      if (event.code === 'BUNDLE_END' && !listening) {
        listening = true
        resolve()
      }
    })
    updateFn && updateFn()
  })
}
