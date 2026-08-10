import { getRuntimeCapabilities, getRuntimeSupport } from 'rolldown/experimental';

// Capability queries against the loaded artifact -- no lane env vars, no
// error-message probing. Safe at module scope: the binding is already loaded
// at collection time.
const capabilities = getRuntimeCapabilities();

// Native builds report the configured shared-runtime executor; every
// WebAssembly build is 'CurrentThread'.
export const runtimeFlavor: string = capabilities.flavor;

// Everything scheduled on the calling thread: native with
// ROLLDOWN_RUNTIME=single, or any WASI build.
export const isSingleThread: boolean = !capabilities.threads;

// WebAssembly/WASI artifact ('wasi' or 'wasi-threads' target) -- distinct from a
// native binding in single-thread mode. Gates wasm-boundary skips (watch,
// symlink traversal).
export const isWasiTest: boolean = capabilities.wasi;

// Threadless WASI only ('wasi' target without threads): neither `isWasiTest`
// (threaded WASI is wasm too) nor `isSingleThread` (native CurrentThread is
// threadless but not wasm). Same field `src/utils/threadless-free.ts` reads, so
// eager-free assertions stay in lockstep with the package.
export const isThreadlessWasi: boolean = getRuntimeSupport(capabilities).threadlessWasi;

// True for every current binding (the shared runtime is the only backend);
// false only when the compat shim synthesized a legacy tokio-era report.
export const isAsyncRuntimeBuild: boolean = capabilities.asyncRuntimeBuild;
