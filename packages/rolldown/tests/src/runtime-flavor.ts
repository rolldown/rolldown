import { getRuntimeCapabilities, getRuntimeSupport } from 'rolldown/experimental';

// Capability queries against the loaded artifact -- no lane env vars, no
// error-message probing. Safe at module scope: the binding is already loaded
// at collection time.
const capabilities = getRuntimeCapabilities();

// The configured shared-runtime executor. Every WebAssembly build defaults to
// 'CurrentThread'; threaded WASI can opt in to 'MultiThread'.
export const runtimeFlavor: string = capabilities.flavor;

// Everything scheduled on the calling thread: native with
// ROLLDOWN_RUNTIME=single, or a WASI build on its default flavor.
export const isSingleThread: boolean = !capabilities.threads;

// WebAssembly/WASI artifact ('wasi' or 'wasi-threads' target) -- distinct from a
// native binding in single-thread mode. Gates wasm-boundary skips (watch,
// symlink traversal, parallel plugins): these stay false on every WASI
// artifact, including threaded WASI on MultiThread, so a test that needs one
// of them gates on this as well as on `isSingleThread`.
export const isWasiTest: boolean = capabilities.wasi;

// Threadless WASI only ('wasi' target without threads): neither `isWasiTest`
// (threaded WASI is wasm too) nor `isSingleThread` (native CurrentThread is
// threadless but not wasm). Same field `src/utils/threadless-free.ts` reads, so
// eager-free assertions stay in lockstep with the package.
export const isThreadlessWasi: boolean = getRuntimeSupport().threadlessWasi;
