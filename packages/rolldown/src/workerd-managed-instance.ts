/**
 * The instance handle `@rolldown/browser/workerd` exposes.
 *
 * The `@napi-rs/cli` deferred loader owns everything below the N-API boundary:
 * module validation, a fresh claimed Memory per instance, per-instance
 * CurrentThread hosts, the settlement barrier and a retryable `dispose()`.
 * This layer adds only what the loader cannot know: whether a build is using
 * the instance.
 *
 * See internal-docs/workerd-managed-instance/implementation.md.
 */
import {
  createInstance as createDeferredInstance,
  type WasiBinding,
  type WasiInstance,
  type WasiModuleInput,
} from './rolldown-binding.wasip1-deferred.js';
import { __enterWorkerdBinding, __exitWorkerdBinding } from './binding-workerd-proxy';

/** A precompiled threadless Rolldown Wasm module, or a promise for one. */
export type WorkerdModuleInput = WasiModuleInput;

/**
 * The size of the linear memory the instance allocates. Omitted, it is
 * `initialPages..maximumPages` of the loader's compiled descriptor.
 */
export interface WorkerdInstanceOptions {
  initialMemoryPages?: number;
  maximumMemoryPages?: number;
}

/** One independent workerd Rolldown instance. */
export interface WorkerdRolldownInstance {
  /** This instance's linear memory. Throws once disposal has completed. */
  readonly memory: WebAssembly.Memory;
  /** Current linear-memory size; 0 once `dispose()` has completed. */
  readonly memoryBytes: number;
  readonly disposed: boolean;
  /**
   * Destroy this instance's N-API environment. Rejects while a build uses the
   * instance. If cleanup rejects, the handle stays undisposed and a later call
   * retries it.
   */
  dispose(): Promise<void>;
}

const DISPOSED = 'This workerd Rolldown instance has been disposed';
const DISPOSING = 'This workerd Rolldown instance is unavailable because disposal has started';

interface InstanceState {
  // Dropped once disposal completes, so a retained handle keeps neither the
  // exports nor the linear memory alive.
  loader: WasiInstance | undefined;
  active: number;
  disposalStarted: boolean;
  disposing: Promise<void> | undefined;
}

// Only this module can count operations; the public handle carries no counter.
const states = new WeakMap<WorkerdRolldownInstance, InstanceState>();

function stateOf(instance: WorkerdRolldownInstance): InstanceState {
  const state = states.get(instance);
  if (state === undefined) {
    throw new TypeError(
      'Expected a workerd Rolldown instance created by createInstance(); ' +
        'pass it as the `instance` option or pass a compiled Wasm `module` instead',
    );
  }
  return state;
}

function usableLoader(state: InstanceState): WasiInstance {
  if (state.loader === undefined) throw new Error(DISPOSED);
  if (state.disposalStarted) throw new Error(DISPOSING);
  return state.loader;
}

/**
 * @internal The raw binding exports, kept off the public handle: anything
 * taken from them outside a build is not counted by `dispose()`, and once held
 * past disposal keeps the dead instance's memory alive. Throws once disposal
 * has started.
 */
export function instanceExports(instance: WorkerdRolldownInstance): WasiBinding {
  return usableLoader(stateOf(instance)).exports;
}

/**
 * @internal Start one build on `instance`: refuse a disposed one, make it the
 * active binding of this module copy, and count it until
 * {@linkcode exitInstance}.
 */
export function enterInstance(instance: WorkerdRolldownInstance): void {
  // Throws while another instance is the active binding.
  __enterWorkerdBinding(instanceExports(instance));
  stateOf(instance).active += 1;
}

/** @internal End one build started by {@linkcode enterInstance}. */
export function exitInstance(instance: WorkerdRolldownInstance): void {
  const state = stateOf(instance);
  if (state.active === 0) return;
  state.active -= 1;
  // `dispose()` refuses while `active > 0`, so the loader is still present.
  __exitWorkerdBinding(state.loader!.exports);
}

/**
 * @internal Dispose a private `build({ module })` instance, retrying the
 * loader's retryable ERR_NAPI_WASI_CLEANUP_PENDING one macrotask apart. A
 * settlement still queued after three drains is stuck: the last error is
 * thrown and the handle is dropped. A completed loader `dispose()` destroys
 * the environment and drops this handle's reference; the Memory itself is
 * reclaimed only when the host garbage-collects it, which workerd rarely does.
 * A dropped stuck instance is reclaimed the same way, so holding it here would
 * not free it sooner; it would only add one more retry.
 */
export async function disposePrivateInstance(instance: WorkerdRolldownInstance): Promise<void> {
  let error: unknown;
  for (let attempt = 0; attempt < 3; attempt += 1) {
    if (attempt > 0) await new Promise((resolve) => setTimeout(resolve, 0));
    try {
      return await instance.dispose();
    } catch (cause) {
      error = cause;
    }
  }
  throw error;
}

/**
 * Create one independent instance on a linear memory no other instance has
 * used. Dispose it once no build uses it; disposal is asynchronous and
 * idempotent once it has completed.
 */
export async function createInstance(
  wasmInput: WorkerdModuleInput,
  options?: WorkerdInstanceOptions,
): Promise<WorkerdRolldownInstance> {
  // Only the page counts reach the loader: a caller Memory could be handed to
  // a second bundled copy of this entry, whose loader would not know it is
  // already in use.
  const loader = await createDeferredInstance(await wasmInput, {
    initialMemoryPages: options?.initialMemoryPages,
    maximumMemoryPages: options?.maximumMemoryPages,
  });
  const state: InstanceState = {
    loader,
    active: 0,
    disposalStarted: false,
    disposing: undefined,
  };
  const instance: WorkerdRolldownInstance = Object.freeze({
    get memory(): WebAssembly.Memory {
      if (state.loader === undefined) throw new Error(DISPOSED);
      return state.loader.memory;
    },
    get memoryBytes(): number {
      return state.loader === undefined ? 0 : state.loader.memoryBytes;
    },
    get disposed(): boolean {
      return state.loader === undefined;
    },
    dispose(): Promise<void> {
      if (state.loader === undefined) return Promise.resolve();
      if (state.disposing !== undefined) return state.disposing;
      if (state.active > 0) {
        const n = state.active;
        return Promise.reject(
          new Error(
            `Cannot dispose this workerd Rolldown instance with ${n} active binding ` +
              `operation${n === 1 ? '' : 's'}; await active operations and close every ` +
              'binding object first',
          ),
        );
      }
      // Set before the loader tears the environment down, so no new build can
      // enter a half-destroyed instance.
      state.disposalStarted = true;
      const disposing = state.loader.dispose().then(
        () => {
          state.loader = undefined;
        },
        (error: unknown) => {
          // The loader keeps a failed disposal retryable; so does this handle.
          state.disposing = undefined;
          throw error;
        },
      );
      state.disposing = disposing;
      return disposing;
    },
  });
  states.set(instance, state);
  return instance;
}
