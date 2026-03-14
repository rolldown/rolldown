import { instantiateNapiModuleSync, MessageHandler, WASI, createFsProxy } from '@napi-rs/wasm-runtime'
import { memfsExported as __memfsExported } from '@napi-rs/wasm-runtime/fs'

const fs = createFsProxy(__memfsExported)

// Collect stderr output (e.g. Rust panic messages) to surface as a descriptive error
const stderrMessages = []

const handler = new MessageHandler({
  onLoad({ wasmModule, wasmMemory }) {
    const wasi = new WASI({
      fs,
      preopens: {
        '/': '/',
      },
      print: function () {
        // eslint-disable-next-line no-console
        console.log.apply(console, arguments)
      },
      printErr: function() {
        stderrMessages.push(Array.from(arguments).join(' '))
        // eslint-disable-next-line no-console
        console.error.apply(console, arguments)
      },
    })
    return instantiateNapiModuleSync(wasmModule, {
      childThread: true,
      wasi,
      overwriteImports(importObject) {
        importObject.env = {
          ...importObject.env,
          ...importObject.napi,
          ...importObject.emnapi,
          memory: wasmMemory,
        }
      },
    })
  },
  onError(err) {
    // Build a descriptive error message from captured stderr output (Rust panic messages)
    // so that the main thread sees the actual panic details rather than a generic
    // "RuntimeError: unreachable executed" message.
    const panicMessage = stderrMessages.filter(Boolean).join('\n')
    const message = panicMessage
      ? `${panicMessage}\n\n(Caused by: ${err.message})`
      : err.message
    self.reportError(new Error(message))
  },
})

globalThis.onmessage = function (e) {
  handler.handle(e)
}
