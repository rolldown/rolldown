import type * as RolldownBinding from './rolldown-binding.wasip1.cjs';

export interface DeferredInstanceOptions {
  /**
   * Caller-provided memory is single-use for managed initialization attempts.
   * Once validation succeeds and initialization begins, it cannot be passed
   * to another createInstance call, even if initialization fails or the
   * created instance is later disposed.
   */
  memory?: WebAssembly.Memory;
  initialMemoryPages?: number;
  maximumMemoryPages?: number;
}

export interface DeferredRuntimeStats {
  /** Counts instances created by this evaluated loader module, not process-wide instances. */
  createdInstances: number;
  /** Counts live instances created by this evaluated loader module. */
  liveInstances: number;
  declaredInitialMemoryBytes: number;
}

export type DeferredRolldownBinding = Omit<
  typeof RolldownBinding,
  | 'registerCurrentThreadTaskHost'
  | 'registerTimerHost'
  | 'unregisterCurrentThreadTaskHost'
  | 'unregisterTimerHost'
>;

export interface DeferredRolldownInstance {
  readonly exports: DeferredRolldownBinding;
  readonly memory: WebAssembly.Memory;
  readonly memoryBytes: number;
  readonly disposed: boolean;
  /**
   * Destroy this instance's N-API environment. Active binding operations and
   * unclosed binding objects reject disposal before cleanup begins. If cleanup
   * throws, the handle remains undisposed and a later call retries cleanup.
   */
  dispose(): void;
}

export const WORKERD_WASM_MEMORY: Readonly<{
  initialPages: number;
  maximumPages: number;
  pageBytes: number;
  initialBytes: number;
  maximumBytes: number;
}>;

export function getDeferredRuntimeStats(): Readonly<DeferredRuntimeStats>;

export function createInstance(
  module: WebAssembly.Module | PromiseLike<WebAssembly.Module>,
  options?: DeferredInstanceOptions,
): Promise<DeferredRolldownInstance>;

export const instantiate: typeof createInstance;
