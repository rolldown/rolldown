// `@napi-rs/cli` renders the deferred loader this entry builds on.
// See internal-docs/workerd-managed-instance/implementation.md.
export {
  createInstance,
  type WorkerdInstanceOptions,
  type WorkerdModuleInput,
  type WorkerdRolldownInstance,
} from './workerd-managed-instance';
export { build, type WorkerdBuildOptions } from './workerd-build';
/**
 * Loader-local instance counts and the declared initial Wasm address space.
 * Use platform memory telemetry for committed memory and quota enforcement.
 */
export {
  getDeferredRuntimeStats as getWorkerdRuntimeStats,
  type WasiRuntimeStats as WorkerdRuntimeStats,
} from './rolldown-binding.wasip1-deferred.js';
export type { InputOptions } from './options/input-options';
export type { OutputOptions } from './options/output-options';
export type { RolldownOutput, OutputChunk, OutputAsset } from './types/rolldown-output';
export type { BundleError } from './utils/error';
export type { Plugin, RolldownPlugin } from './plugin';
