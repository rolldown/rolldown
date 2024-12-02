import { expect, test, vi, afterEach } from 'vitest'
import { watch, RolldownWatcher } from 'rolldown'
import fs from 'node:fs'
import path from 'node:path'
import { sleep } from '@tests/utils'

const input = path.join(import.meta.dirname, './main.js')
const output = path.join(import.meta.dirname, './dist/main.js')

afterEach(async () => {
  // revert change
  fs.writeFileSync(input, 'console.log(1)\n')
  // TODO: find a way to avoid emit the change event at next test
  await sleep(60)
})

test.sequential('watch', async () => {
  const watchChangeFn = vi.fn()
  const closeWatcherFn = vi.fn()
  const watcher = watch({
    input,
    cwd: import.meta.dirname,
    plugins: [
      {
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

test.sequential('watch close', async () => {
  const watcher = watch({
    input,
    cwd: import.meta.dirname,
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
  const watcher = watch({
    input,
    cwd: import.meta.dirname,
  })

  const events: any[] = []
  watcher.on('event', (event) => {
    if (event.code === 'BUNDLE_END') {
      expect(event.output).toEqual([path.join(import.meta.dirname, './dist')])
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
    expect(events.slice(0, 4)).toEqual([
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
    // The different platform maybe emit multiple events, so here only check the first 4 events
    expect(events.slice(0, 4)).toEqual([
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

test.sequential('watch event avoid deadlock #2806', async () => {
  const watcher = watch({
    input,
    cwd: import.meta.dirname,
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
  const dir = path.join(import.meta.dirname, './skipWrite-dist/')
  const watcher = watch({
    input,
    cwd: import.meta.dirname,
    output: {
      dir,
    },
    watch: {
      skipWrite: true,
    },
  })
  await waitUtil(() => {
    expect(fs.existsSync(dir)).toBe(false)
  })
  await watcher.close()
})

test.sequential('PluginContext addWatchFile', async () => {
  const foo = path.join(import.meta.dirname, './foo.js')
  const watcher = watch({
    input,
    cwd: import.meta.dirname,
    plugins: [
      {
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
  const watcher = watch({
    input,
    cwd: import.meta.dirname,
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
  fs.writeFileSync(input, 'conso le.log(1)')
  // wait 60ms avoid the change event emit at first build
  await new Promise((resolve) => {
    setTimeout(resolve, 60)
  })
  const watcher = watch({
    input,
    cwd: import.meta.dirname,
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
  const watcher = watch({
    input,
    cwd: import.meta.dirname,
    plugins: [
      {
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

async function waitUtil(expectFn: () => void) {
  for (let tries = 0; tries < 10; tries++) {
    try {
      await expectFn()
      return
    } catch {}
    await sleep(50)
  }
  expectFn()
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
