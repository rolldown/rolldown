import {
  emnapiAsyncWorkPlugin as __emnapiAsyncWorkPlugin,
  emnapiTSFNPlugin as __emnapiTSFNPlugin,
  instantiateNapiModule as __emnapiInstantiateNapiModule,
  WASI as __WASI,
} from '@napi-rs/wasm-runtime'
import { createContext as __emnapiCreateContext } from '@emnapi/runtime'
import { memfs, Buffer } from '@napi-rs/wasm-runtime/fs'


export const { fs: __fs, vol: __volume } = memfs()

const __wasi = new __WASI({
  version: 'preview1',
  fs: __fs,
  preopens: {
    '/': '/',
  },
})

const __wasmUrl = new URL('./rolldown-binding.wasm32-wasip1.wasm', import.meta.url).href
const __wasmResponse = await globalThis.fetch(__wasmUrl)
if (!__wasmResponse.ok) {
  throw new Error(
    'Failed to fetch WASI module ' +
      __wasmUrl +
      ': ' +
      __wasmResponse.status +
      ' ' +
      (__wasmResponse.statusText || 'Unknown Status'),
  )
}
const __wasmFile = await __wasmResponse.arrayBuffer()

const __wasmMemory = new WebAssembly.Memory({
  initial: 1024,
  maximum: 65536,
})
let __emnapiContext

const __wasiDisposeSymbol = Symbol.for('napi.rs.wasi.dispose')
const __wasiWorkers = new Set()
let __napiInstance
let __emnapiContextDestroyed = false
let __emnapiContextDestroyPromise
let __emnapiWasmEnvCleanupPrepared = false
let __emnapiWasmEnvCleanupRan = false
let __emnapiWasmEnvCleanupDrained = false
let __emnapiWasmEnvCleanupDrainPromise
let __wasiDisposed = false
let __wasiDisposePromise
let __completeWasiDisposal = function() {}
// Overridden by loader flavors that have a last-resort reclaim for a rollback
// that stopped short of destroying the context. See
// `__rollbackWasiInitialization`.
let __retainWasiRollbackForRetry = function() {}

function __isThenable(value) {
  return (
    value !== null &&
    (typeof value === 'object' || typeof value === 'function') &&
    typeof value.then === 'function'
  )
}

function __createCleanupError(errors, message) {
  if (errors.length === 1) {
    return errors[0]
  }
  const __AggregateError = globalThis.AggregateError
  if (typeof __AggregateError === 'function') {
    return new __AggregateError(errors, message)
  }
  const error = new Error(message)
  error.errors = errors
  return error
}

function __attachCleanupErrors(error, cleanupErrors) {
  if (cleanupErrors.length === 0) {
    return error
  }
  const cleanupError = __createCleanupError(
    cleanupErrors,
    'WASI binding cleanup failed',
  )
  try {
    if (
      error &&
      (typeof error === 'object' || typeof error === 'function')
    ) {
      if (error.cause === undefined) {
        error.cause = cleanupError
        if (error.cause === cleanupError) {
          return error
        }
      }
      if (Array.isArray(error.cleanupErrors)) {
        error.cleanupErrors.push(cleanupError)
        return error
      } else {
        const attachedCleanupErrors = [cleanupError]
        error.cleanupErrors = attachedCleanupErrors
        if (error.cleanupErrors === attachedCleanupErrors) {
          return error
        }
      }
    }
  } catch {}
  const aggregate = __createCleanupError(
    [error, cleanupError],
    'WASI binding initialization and cleanup failed',
  )
  try {
    aggregate.cause = error
  } catch {}
  return aggregate
}

function __wrapEmnapiContextDestroyForSettlement(context) {
  // oxlint-disable-next-line typescript/unbound-method -- invoked with the wrapper receiver below
  const __contextDestroy = context.destroy
  context.destroy = function () {
    __prepareWasmEnvCleanup()
    return Reflect.apply(__contextDestroy, this, arguments)
  }
  return context
}

function __prepareWasmEnvCleanup() {
  if (__emnapiWasmEnvCleanupPrepared) {
    return
  }
  const prepare = __napiInstance?.exports?.napi_prepare_wasm_env_cleanup
  if (typeof prepare === 'function') {
    prepare()
    __emnapiWasmEnvCleanupRan = true
  }
  __emnapiWasmEnvCleanupPrepared = true
}

// Mirror the primitive @emnapi/core schedules its threadsafe-function dispatch
// on, so the drain turns below interleave with that dispatch instead of racing
// ahead of it on a faster queue.
const __scheduleMacrotask = (function () {
  if (typeof setImmediate === 'function') {
    return function (callback) {
      setImmediate(callback)
    }
  }
  const __MessageChannel = globalThis.MessageChannel
  if (typeof __MessageChannel === 'function') {
    return function (callback) {
      const channel = new __MessageChannel()
      channel.port1.onmessage = function () {
        channel.port1.onmessage = null
        try {
          channel.port1.close()
        } catch {}
        try {
          channel.port2.close()
        } catch {}
        callback()
      }
      channel.port2.postMessage(null)
    }
  }
  return function (callback) {
    setTimeout(callback, 0)
  }
})()

// Turns to wait for while the addon still reports queued settlements. Reaching
// zero is the only success. A counter still nonzero at this bound rejects the
// disposal as retryable (`ERR_NAPI_WASI_CLEANUP_PENDING`) rather than
// destroying the context over a still-queued settlement — the wait stays
// bounded either way.
const __WASM_ENV_CLEANUP_DRAIN_TURNS = 128
// Without `napi_wasm_env_cleanup_pending` the queue is not observable. Fall
// back to the number of turns @emnapi/core needs to coalesce and dispatch a
// call made on this thread (two), plus a margin.
const __WASM_ENV_CLEANUP_BLIND_DRAIN_TURNS = 4

/**
 * `napi_prepare_wasm_env_cleanup` only *queues* the promise settlements of the
 * tasks it cancelled: `napi_call_threadsafe_function` appends to the
 * threadsafe-function queue, and @emnapi/core dispatches that queue from a
 * macrotask — two coalescing turns later, even for a call made on this very
 * thread. `Context.destroy()` then runs the threadsafe function's cleanup hook,
 * which drains the queue with a null env and *discards* whatever is still in it.
 *
 * So destroying without yielding first strands exactly the promises the barrier
 * exists to settle. Yield real event-loop turns until the addon reports the
 * queue empty; microtask checkpoints cannot help, no number of them lets a
 * macrotask run.
 *
 * Returns nothing when there is nothing to wait for, which keeps disposal
 * synchronous in the common case.
 *
 * The "already drained" flag is set only once a wait has actually finished.
 * Scheduling a macrotask can fail — a host-provided or patched `setImmediate`
 * that throws is enough — and a disposal that rejects stays retryable, so
 * marking the drain complete up front would make the retry skip it and destroy
 * the context with the barrier's settlements still queued.
 *
 * A wait that runs out of turns with the counter still nonzero rejects with
 * `ERR_NAPI_WASI_CLEANUP_PENDING` for the same reason: at that point
 * "finished" is indistinguishable from the stranding above, and destroying
 * would discard the very settlement the wait was for. The rejection leaves the
 * flag unset and disposal retryable.
 */
function __drainWasmEnvCleanup() {
  if (__emnapiWasmEnvCleanupDrained || !__emnapiWasmEnvCleanupRan) {
    return
  }
  if (__emnapiWasmEnvCleanupDrainPromise) {
    return __emnapiWasmEnvCleanupDrainPromise
  }
  const pending = __napiInstance?.exports?.napi_wasm_env_cleanup_pending
  const observable = typeof pending === 'function'
  if (observable) {
    let queued
    try {
      queued = pending()
    } catch {
      __emnapiWasmEnvCleanupDrained = true
      return
    }
    if (!queued) {
      __emnapiWasmEnvCleanupDrained = true
      return
    }
  }
  const limit = observable
    ? __WASM_ENV_CLEANUP_DRAIN_TURNS
    : __WASM_ENV_CLEANUP_BLIND_DRAIN_TURNS
  const drainPromise = (async () => {
    let queued = 0
    for (let turn = 0; turn < limit; turn++) {
      await new Promise((resolve) => {
        __scheduleMacrotask(resolve)
      })
      if (!observable) {
        continue
      }
      try {
        queued = pending()
      } catch {
        return
      }
      if (!queued) {
        return
      }
    }
    if (!observable) {
      // Blind wait: without `napi_wasm_env_cleanup_pending` the bound IS the
      // contract — there is nothing to consult, so finishing the turns is
      // finishing the drain.
      return
    }
    // The counter is still nonzero after every turn the bound allows. The wait
    // stays bounded — but claiming success here would be indistinguishable from
    // the stranding this drain exists to prevent: disposal would go on to
    // destroy the context, whose cleanup hook discards the still-queued
    // settlement with a null env, and the promise it was for hangs forever.
    // Reject instead, as a retryable cleanup failure: the drained flag stays
    // unset, dispose() (and the rollback) decline to destroy, and a later
    // dispose() runs the drain again — by which time the queue has usually been
    // delivered. A counter that is somehow stuck nonzero therefore costs each
    // attempt at most another bounded wait and a rejection, never a stranded
    // promise; the process-exit teardown still reclaims the context.
    const drainError = new Error(
      'the wasm environment still reports ' +
        queued +
        ' queued settlement(s) after ' +
        limit +
        ' event-loop turns; the context was not destroyed - retry dispose() to wait for the queue again',
    )
    drainError.code = 'ERR_NAPI_WASI_CLEANUP_PENDING'
    throw drainError
  })().then(
    (value) => {
      // Set only when the wait actually finished AND the queue was seen empty
      // (or is unobservable): a drain that timed out with settlements still
      // queued rejects above and must stay repeatable.
      __emnapiWasmEnvCleanupDrained = true
      __emnapiWasmEnvCleanupDrainPromise = undefined
      return value
    },
    (error) => {
      __emnapiWasmEnvCleanupDrainPromise = undefined
      throw error
    },
  )
  __emnapiWasmEnvCleanupDrainPromise = drainPromise
  return drainPromise
}

function __destroyEmnapiContext() {
  if (__emnapiContextDestroyed || __emnapiContext === undefined) {
    __emnapiContextDestroyed = true
    return
  }
  if (__emnapiContextDestroyPromise) {
    return __emnapiContextDestroyPromise
  }

  __prepareWasmEnvCleanup()
  const result = __emnapiContext.destroy()
  if (!__isThenable(result)) {
    __emnapiContextDestroyed = true
    return
  }

  const destroyPromise = Promise.resolve(result).then(
    (value) => {
      __emnapiContextDestroyed = true
      return value
    },
    (error) => {
      __emnapiContextDestroyPromise = undefined
      throw error
    },
  )
  __emnapiContextDestroyPromise = destroyPromise
  return destroyPromise
}

function __terminateWasiWorkers() {
  const cleanupErrors = []
  const pending = []

  for (const worker of __wasiWorkers) {
    let result
    try {
      result = worker.terminate()
    } catch (error) {
      cleanupErrors.push(error)
      continue
    }
    if (__isThenable(result)) {
      pending.push(
        Promise.resolve(result).then(
          () => {
            __wasiWorkers.delete(worker)
          },
          (error) => {
            cleanupErrors.push(error)
          },
        ),
      )
    } else {
      __wasiWorkers.delete(worker)
    }
  }

  const finish = () => {
    if (cleanupErrors.length > 0) {
      throw __createCleanupError(
        cleanupErrors,
        'Failed to terminate WASI workers',
      )
    }
  }
  return pending.length > 0 ? Promise.all(pending).then(finish) : finish()
}

function __finishWasiDisposal() {
  const workerResult = __terminateWasiWorkers()
  if (__isThenable(workerResult)) {
    return Promise.resolve(workerResult).then(__completeWasiDisposal)
  }
  return __completeWasiDisposal()
}

function __continueWasiDisposal() {
  const destroyResult = __destroyEmnapiContext()
  if (__isThenable(destroyResult)) {
    return Promise.resolve(destroyResult).then(__finishWasiDisposal)
  }
  return __finishWasiDisposal()
}

function __startWasiDisposal() {
  // Run the pre-teardown barrier, then let the settlements it queued actually
  // reach JavaScript, and only then destroy the environment. Doing these two
  // back to back is what strands them.
  __prepareWasmEnvCleanup()
  const drainResult = __drainWasmEnvCleanup()
  if (__isThenable(drainResult)) {
    return Promise.resolve(drainResult).then(__continueWasiDisposal)
  }
  return __continueWasiDisposal()
}

/**
 * Disposes this generated WASI binding.
 *
 * Access this function with:
 * binding[Symbol.for('napi.rs.wasi.dispose')]()
 */
function __disposeWasiBinding() {
  if (__wasiDisposePromise) {
    return __wasiDisposePromise
  }
  if (__wasiDisposed) {
    return Promise.resolve()
  }

  let resolveDispose
  let rejectDispose
  const disposePromise = new Promise((resolve, reject) => {
    resolveDispose = resolve
    rejectDispose = reject
  })
  __wasiDisposePromise = disposePromise

  let result
  try {
    result = __startWasiDisposal()
  } catch (error) {
    __wasiDisposePromise = undefined
    rejectDispose(error)
    return disposePromise
  }

  Promise.resolve(result).then(
    (value) => {
      __wasiDisposed = true
      resolveDispose(value)
    },
    (error) => {
      __wasiDisposePromise = undefined
      rejectDispose(error)
    },
  )
  return disposePromise
}

function __publishWasiDispose(exports) {
  Object.defineProperty(exports, __wasiDisposeSymbol, {
    configurable: false,
    enumerable: false,
    value: __disposeWasiBinding,
    writable: false,
  })
}

function __finishWasiInitializationRollback(cleanupErrors) {
  let workerResult
  try {
    workerResult = __terminateWasiWorkers()
  } catch (cleanupError) {
    cleanupErrors.push(cleanupError)
    return cleanupErrors
  }
  if (__isThenable(workerResult)) {
    return Promise.resolve(workerResult)
      .catch((cleanupError) => {
        cleanupErrors.push(cleanupError)
      })
      .then(() => cleanupErrors)
  }
  return cleanupErrors
}

function __destroyContextForWasiRollback(cleanupErrors) {
  let destroyResult
  try {
    destroyResult = __destroyEmnapiContext()
  } catch (cleanupError) {
    cleanupErrors.push(cleanupError)
    return __finishWasiInitializationRollback(cleanupErrors)
  }
  if (__isThenable(destroyResult)) {
    return Promise.resolve(destroyResult)
      .catch((cleanupError) => {
        cleanupErrors.push(cleanupError)
      })
      .then(() => __finishWasiInitializationRollback(cleanupErrors))
  }
  return __finishWasiInitializationRollback(cleanupErrors)
}

/**
 * Leaves a rollback that could not reach the queued settlements undestroyed, and
 * hands it to whatever this flavor has that can still reclaim it.
 */
function __retainFailedWasiRollback(cleanupErrors) {
  try {
    __retainWasiRollbackForRetry()
  } catch (cleanupError) {
    cleanupErrors.push(cleanupError)
  }
  return cleanupErrors
}

/**
 * Initialization can fail *after* registration has already run, and registration
 * runs with a live environment: a module-init hook can start async work and then
 * return an error, and the promise it created may already have escaped into
 * JavaScript. The barrier cancels that work and *queues* the settlement, so this
 * path needs the same drain the ordinary disposal does — destroying without
 * yielding discards the queue with a null env and strands the promise.
 *
 * Stays synchronous when nothing is queued, which covers every failure before
 * `beforeInit`: there is no instance to run the barrier on, so nothing to drain.
 *
 * A barrier or drain that did *not* finish stops the rollback short of
 * destroying, which is what `dispose()` already does — a rejected drain there
 * never reaches `__continueWasiDisposal`. Destroying anyway is the worse of the
 * two trades, and not because of what it saves:
 *
 *   - It cannot deliver the settlements. `Context.destroy()` runs the
 *     threadsafe function's cleanup hook, which drains the queue with a null env
 *     and discards it, so a promise that already escaped into JavaScript hangs
 *     forever with nothing left that could ever settle it.
 *   - It saves less than it looks. `Context.destroy()` stops JavaScript calls
 *     and runs cleanup hooks; it does not free the wasm instance or its Memory,
 *     which this module's scope holds either way. What stopping short retains is
 *     the emnapi context's bookkeeping and its un-run cleanup hooks.
 *   - Retry is not theoretical. A rollback that records a cleanup error is
 *     already kept in the process-wide registry above, so re-`require()`ing this
 *     file replays it instead of re-instantiating — and the `6e15de6f` flag fix
 *     means the replay drains again rather than skipping it. Destroying first is
 *     what makes that retained record useless.
 *
 * The residual cost is honest: the CJS flavor hands the context to its
 * `process.on('exit')` teardown, so a process that never retries still reclaims
 * it on the way out. The ESM browser flavor has no equivalent — a module that
 * throws while evaluating is permanently errored, so re-importing rethrows
 * without re-running this file — and there the context stays until the realm
 * goes away. That is the deliberate choice: a hung promise is a silent liveness
 * bug with no upper bound, while the retained bookkeeping is bounded by the page.
 */
function __rollbackWasiInitialization() {
  const cleanupErrors = []
  let drainResult
  let settlementsUnreached = false
  try {
    __prepareWasmEnvCleanup()
    drainResult = __drainWasmEnvCleanup()
  } catch (cleanupError) {
    cleanupErrors.push(cleanupError)
    settlementsUnreached = true
  }
  if (__isThenable(drainResult)) {
    return Promise.resolve(drainResult).then(
      () => __destroyContextForWasiRollback(cleanupErrors),
      (cleanupError) => {
        cleanupErrors.push(cleanupError)
        return __retainFailedWasiRollback(cleanupErrors)
      },
    )
  }
  if (settlementsUnreached) {
    return __retainFailedWasiRollback(cleanupErrors)
  }
  return __destroyContextForWasiRollback(cleanupErrors)
}

let __browserTaskHostRegistration
let __browserTimerHostRegistration
let __wasiModule
let __napiModule

try {
/* ROLLDOWN_BROWSER_INITIALIZATION_GUARD_START */
  __emnapiContext = __wrapEmnapiContextDestroyForSettlement(__emnapiCreateContext({ autoDestroy: false }))
  __emnapiContext.suppressDestroy()
    __emnapiContext.features.Buffer = Buffer

  ;({
    instance: __napiInstance,
    module: __wasiModule,
    napiModule: __napiModule,
  } = await __emnapiInstantiateNapiModule(__wasmFile, {
    context: __emnapiContext,
    asyncWorkPoolSize: 0,
    plugins: [__emnapiAsyncWorkPlugin, __emnapiTSFNPlugin],
    wasi: __wasi,
    overwriteImports(importObject) {
      importObject.env = {
        ...importObject.env,
        ...importObject.napi,
        ...importObject.emnapi,
        memory: __wasmMemory,
      }
      return importObject
    },
    beforeInit({ instance }) {
      __napiInstance = instance
      for (const name of Object.keys(instance.exports)) {
        if (name.startsWith('__napi_register__')) {
          instance.exports[name]()
        }
      }
    },
  }))
  __publishWasiDispose(__napiModule.exports)
/* ROLLDOWN_CURRENT_THREAD_HOST_BOOTSTRAP_START */
;{
  const __rolldownBinding = __napiModule.exports
  const __getCurrentThreadTaskHostContractVersion =
    __rolldownBinding.getCurrentThreadTaskHostContractVersion
  const __isCurrentThreadHostRegistrationActive =
    __rolldownBinding.isCurrentThreadHostRegistrationActive
  const __registerCurrentThreadTaskHost =
    __rolldownBinding.registerCurrentThreadTaskHost
  const __registerTimerHost = __rolldownBinding.registerTimerHost
  const __reserveCurrentThreadHostRegistration =
    __rolldownBinding.reserveCurrentThreadHostRegistration
  const __unregisterCurrentThreadTaskHost =
    __rolldownBinding.unregisterCurrentThreadTaskHost
  const __unregisterTimerHost = __rolldownBinding.unregisterTimerHost
  if (
    typeof __getCurrentThreadTaskHostContractVersion !== 'function' ||
    typeof __isCurrentThreadHostRegistrationActive !== 'function' ||
    typeof __registerCurrentThreadTaskHost !== 'function' ||
    typeof __registerTimerHost !== 'function' ||
    typeof __reserveCurrentThreadHostRegistration !== 'function' ||
    typeof __unregisterCurrentThreadTaskHost !== 'function' ||
    typeof __unregisterTimerHost !== 'function'
  ) {
    throw new TypeError(
      'The threadless Rolldown binding does not expose its CurrentThread host integration',
    )
  }
  const __taskHostContractVersion =
    Reflect.apply(
      __getCurrentThreadTaskHostContractVersion,
      __rolldownBinding,
      [],
    )
  if (__taskHostContractVersion !== 4) {
    throw new TypeError(
      'The threadless Rolldown binding uses CurrentThread task-host contract version ' +
        String(__taskHostContractVersion) +
        ', but version 4 is required',
    )
  }
  const __readHostRegistration = (__registration, __label) => {
    let __high
    let __low
    try {
      __high = Reflect.get(__registration, 'high', __registration)
      __low = Reflect.get(__registration, 'low', __registration)
    } catch {}
    if (
      !Number.isInteger(__high) ||
      __high < 0 ||
      __high > 0xffffffff ||
      !Number.isInteger(__low) ||
      __low < 0 ||
      __low > 0xffffffff ||
      (__high === 0 && __low === 0)
    ) {
      throw new TypeError(
        'The threadless Rolldown binding returned an invalid ' +
          __label +
          ' host registration',
      )
    }
    return { high: __high, low: __low }
  }
  const __assertHostRegistrationActive = (__registration, __label) => {
    const __active = Reflect.apply(
      __isCurrentThreadHostRegistrationActive,
      __rolldownBinding,
      [__registration.high, __registration.low],
    )
    if (typeof __active !== 'boolean') {
      throw new TypeError(
        'The threadless Rolldown binding returned an invalid ' +
          __label +
          ' host liveness result',
      )
    }
    if (!__active) {
      throw new TypeError(
        'The threadless Rolldown binding returned an inactive ' +
          __label +
          ' host registration',
      )
    }
  }
  const __taskHostRegistration = __readHostRegistration(
    Reflect.apply(
      __reserveCurrentThreadHostRegistration,
      __rolldownBinding,
      [],
    ),
    'task',
  )
  __browserTaskHostRegistration = __taskHostRegistration
  Reflect.apply(__registerCurrentThreadTaskHost, __rolldownBinding, [
    __taskHostRegistration.high,
    __taskHostRegistration.low,
  ])
  __assertHostRegistrationActive(__taskHostRegistration, 'task')

  const __setTimeoutHost = globalThis.setTimeout?.bind(globalThis)
  const __clearTimeoutHost = globalThis.clearTimeout?.bind(globalThis)
  if (__setTimeoutHost && __clearTimeoutHost) {
    const __MAX_HOST_TIMEOUT_MS = 2147483647
    const __activeTimers = new Map()
    const __armTimer = (__id, __timer) => {
      const __delay = Math.min(__timer.remainingMs, __MAX_HOST_TIMEOUT_MS)
      __timer.handle = __setTimeoutHost(() => {
        if (__activeTimers.get(__id) !== __timer) return
        __timer.remainingMs -= __delay
        if (__timer.remainingMs > 0) {
          try {
            __armTimer(__id, __timer)
          } catch (__error) {
            __activeTimers.delete(__id)
            __timer.reject(__error)
          }
          return
        }
        __activeTimers.delete(__id)
        __timer.resolve()
      }, __delay)
    }
    const __cancelTimer = (__timer) => {
      try {
        if (__timer.handle !== undefined) {
          __clearTimeoutHost(__timer.handle)
        }
      } catch {
        // Rust invokes this callback through a non-catching TSFN. Contain
        // host cancellation failures at the JavaScript boundary.
      } finally {
        __timer.resolve()
      }
    }
    const __timerHostRegistration = __readHostRegistration(
      Reflect.apply(
        __reserveCurrentThreadHostRegistration,
        __rolldownBinding,
        [],
      ),
      'timer',
    )
    __browserTimerHostRegistration = __timerHostRegistration
    Reflect.apply(__registerTimerHost, __rolldownBinding, [
      __timerHostRegistration.high,
      __timerHostRegistration.low,
      (__id, __ms) => {
        const __previous = __activeTimers.get(__id)
        if (__previous) {
            __activeTimers.delete(__id)
            __cancelTimer(__previous)
          }
          return new Promise((__resolve, __reject) => {
            const __timer = {
              handle: undefined,
              remainingMs: Math.max(__ms, 0),
              reject: __reject,
              resolve: __resolve,
            }
            __activeTimers.set(__id, __timer)
            try {
              __armTimer(__id, __timer)
            } catch (__error) {
              if (__activeTimers.get(__id) === __timer) {
                __activeTimers.delete(__id)
              }
              __reject(__error)
            }
          })
        },
        (__id) => {
          const __timer = __activeTimers.get(__id)
          if (!__timer) return
          __activeTimers.delete(__id)
          __cancelTimer(__timer)
        },
      ])
    __assertHostRegistrationActive(__timerHostRegistration, 'timer')
  }
}
/* ROLLDOWN_CURRENT_THREAD_HOST_BOOTSTRAP_END */
/* ROLLDOWN_BROWSER_INITIALIZATION_GUARD_END */
} catch (error) {
  const __hostCleanupErrors = []
  const __cleanupSync = (__operation, __message) => {
    const __operationErrors = []
    for (let __attempt = 0; __attempt < 2; __attempt += 1) {
      try {
        __operation()
        return
      } catch (__cleanupError) {
        __operationErrors.push(__cleanupError)
      }
    }
    __hostCleanupErrors.push(new AggregateError(__operationErrors, __message))
  }
  if (__browserTimerHostRegistration !== undefined) {
    __cleanupSync(() => {
      const __binding = __napiModule.exports
      Reflect.apply(__binding.unregisterTimerHost, __binding, [
        __browserTimerHostRegistration.high,
        __browserTimerHostRegistration.low,
      ])
    }, 'Threadless browser timer-host cleanup failed')
  }
  if (__browserTaskHostRegistration !== undefined) {
    __cleanupSync(() => {
      const __binding = __napiModule.exports
      Reflect.apply(__binding.unregisterCurrentThreadTaskHost, __binding, [
        __browserTaskHostRegistration.high,
        __browserTaskHostRegistration.low,
      ])
    }, 'Threadless browser task-host cleanup failed')
  }
  const cleanupErrors = await __rollbackWasiInitialization()
  throw __attachCleanupErrors(error, __hostCleanupErrors.concat(cleanupErrors))
}
export default __napiModule.exports
export const __rolldownBindingTarget = 'wasi'
export const LegalCommentsMode = __napiModule.exports.LegalCommentsMode
export const minify = __napiModule.exports.minify
export const minifySync = __napiModule.exports.minifySync
export const Severity = __napiModule.exports.Severity
export const ParseResult = __napiModule.exports.ParseResult
export const ExportExportNameKind = __napiModule.exports.ExportExportNameKind
export const ExportImportNameKind = __napiModule.exports.ExportImportNameKind
export const ExportLocalNameKind = __napiModule.exports.ExportLocalNameKind
export const ImportNameKind = __napiModule.exports.ImportNameKind
export const parse = __napiModule.exports.parse
export const parseSync = __napiModule.exports.parseSync
export const rawTransferSupported = __napiModule.exports.rawTransferSupported
export const ResolverFactory = __napiModule.exports.ResolverFactory
export const EnforceExtension = __napiModule.exports.EnforceExtension
export const ModuleType = __napiModule.exports.ModuleType
export const sync = __napiModule.exports.sync
export const HelperMode = __napiModule.exports.HelperMode
export const isolatedDeclaration = __napiModule.exports.isolatedDeclaration
export const isolatedDeclarationSync = __napiModule.exports.isolatedDeclarationSync
export const moduleRunnerTransform = __napiModule.exports.moduleRunnerTransform
export const moduleRunnerTransformSync = __napiModule.exports.moduleRunnerTransformSync
export const transform = __napiModule.exports.transform
export const transformSync = __napiModule.exports.transformSync
export const BindingAsyncRuntimeLease = __napiModule.exports.BindingAsyncRuntimeLease
export const BindingBundleEndEventData = __napiModule.exports.BindingBundleEndEventData
export const BindingBundleErrorEventData = __napiModule.exports.BindingBundleErrorEventData
export const BindingBundler = __napiModule.exports.BindingBundler
export const BindingBundleStartEventData = __napiModule.exports.BindingBundleStartEventData
export const BindingCallableBuiltinPlugin = __napiModule.exports.BindingCallableBuiltinPlugin
export const BindingChunkingContext = __napiModule.exports.BindingChunkingContext
export const BindingDecodedMap = __napiModule.exports.BindingDecodedMap
export const BindingDevEngine = __napiModule.exports.BindingDevEngine
export const BindingLoadPluginContext = __napiModule.exports.BindingLoadPluginContext
export const BindingMagicString = __napiModule.exports.BindingMagicString
export const BindingModuleInfo = __napiModule.exports.BindingModuleInfo
export const BindingNormalizedOptions = __napiModule.exports.BindingNormalizedOptions
export const BindingOutputAsset = __napiModule.exports.BindingOutputAsset
export const BindingOutputChunk = __napiModule.exports.BindingOutputChunk
export const BindingPluginContext = __napiModule.exports.BindingPluginContext
export const BindingRenderedChunk = __napiModule.exports.BindingRenderedChunk
export const BindingRenderedChunkMeta = __napiModule.exports.BindingRenderedChunkMeta
export const BindingRenderedModule = __napiModule.exports.BindingRenderedModule
export const BindingSourceMap = __napiModule.exports.BindingSourceMap
export const BindingTransformPluginContext = __napiModule.exports.BindingTransformPluginContext
export const BindingWatcher = __napiModule.exports.BindingWatcher
export const BindingWatcherBundler = __napiModule.exports.BindingWatcherBundler
export const BindingWatcherChangeData = __napiModule.exports.BindingWatcherChangeData
export const BindingWatcherEvent = __napiModule.exports.BindingWatcherEvent
export const ParallelJsPluginRegistry = __napiModule.exports.ParallelJsPluginRegistry
export const TraceSubscriberGuard = __napiModule.exports.TraceSubscriberGuard
export const TsconfigCache = __napiModule.exports.TsconfigCache
export const acquireAsyncRuntime = __napiModule.exports.acquireAsyncRuntime
export const BindingAttachDebugInfo = __napiModule.exports.BindingAttachDebugInfo
export const BindingBuiltinPluginName = __napiModule.exports.BindingBuiltinPluginName
export const BindingChunkModuleOrderBy = __napiModule.exports.BindingChunkModuleOrderBy
export const BindingErrorStage = __napiModule.exports.BindingErrorStage
export const BindingLogLevel = __napiModule.exports.BindingLogLevel
export const BindingPluginOrder = __napiModule.exports.BindingPluginOrder
export const BindingPropertyReadSideEffects = __napiModule.exports.BindingPropertyReadSideEffects
export const BindingPropertyWriteSideEffects = __napiModule.exports.BindingPropertyWriteSideEffects
export const BindingRebuildStrategy = __napiModule.exports.BindingRebuildStrategy
export const BindingRuntimeFlavor = __napiModule.exports.BindingRuntimeFlavor
export const collapseSourcemaps = __napiModule.exports.collapseSourcemaps
export const configureAsyncRuntime = __napiModule.exports.configureAsyncRuntime
export const enhancedTransform = __napiModule.exports.enhancedTransform
export const enhancedTransformSync = __napiModule.exports.enhancedTransformSync
export const FilterTokenKind = __napiModule.exports.FilterTokenKind
export const getAsyncRuntimeConfig = __napiModule.exports.getAsyncRuntimeConfig
export const getAsyncRuntimeMetrics = __napiModule.exports.getAsyncRuntimeMetrics
export const getCurrentThreadTaskHostContractVersion = __napiModule.exports.getCurrentThreadTaskHostContractVersion
export const getNativeMemoryStats = __napiModule.exports.getNativeMemoryStats
export const getRuntimeCapabilities = __napiModule.exports.getRuntimeCapabilities
export const initTraceSubscriber = __napiModule.exports.initTraceSubscriber
export const isCurrentThreadHostRegistrationActive = __napiModule.exports.isCurrentThreadHostRegistrationActive
export const registerCurrentThreadTaskHost = __napiModule.exports.registerCurrentThreadTaskHost
export const registerPlugins = __napiModule.exports.registerPlugins
export const registerTimerHost = __napiModule.exports.registerTimerHost
export const reserveCurrentThreadHostRegistration = __napiModule.exports.reserveCurrentThreadHostRegistration
export const resetAsyncRuntimeMetrics = __napiModule.exports.resetAsyncRuntimeMetrics
export const resetNativeMemoryStats = __napiModule.exports.resetNativeMemoryStats
export const resolveTsconfig = __napiModule.exports.resolveTsconfig
export const shutdownAsyncRuntime = __napiModule.exports.shutdownAsyncRuntime
export const startAsyncRuntime = __napiModule.exports.startAsyncRuntime
export const unregisterCurrentThreadTaskHost = __napiModule.exports.unregisterCurrentThreadTaskHost
export const unregisterTimerHost = __napiModule.exports.unregisterTimerHost
