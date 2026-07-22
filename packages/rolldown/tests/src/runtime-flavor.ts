import { getRuntimeCapabilities } from 'rolldown/experimental';

// The binding self-reports what it IS (backend / flavor / target) through
// `getRuntimeCapabilities()`: compile-time facts plus the config snapshot
// resolved once at addon load. The helpers below are capability queries
// against the loaded artifact -- nothing here reads lane env vars or probes
// error messages. Querying at module scope is safe in every lane: this module
// already loaded the binding at collection time for `runtimeFlavor` before.
const capabilities = getRuntimeCapabilities();

// Flavor is reported by every build: native builds report the configured
// shared-runtime executor; every WebAssembly build is 'CurrentThread'.
export const runtimeFlavor: string = capabilities.flavor;

// True when the binding schedules everything on the calling thread
// (native build with ROLLDOWN_RUNTIME=single, or any WASI build).
// Equivalent to `capabilities.flavor === 'CurrentThread'`.
export const isSingleThread: boolean = !capabilities.threads;

// True when the loaded binding is a WebAssembly/WASI artifact ('wasi' or
// 'wasi-threads' target) -- distinguishes "wasm binding" from "native binding
// in single-thread mode". Gates wasm-boundary-specific skips (watch and
// symlink traversal); no CI env var is involved, the artifact identifies itself.
export const isWasiTest: boolean = capabilities.wasi;

// True for every current binding (the shared runtime is the only backend);
// false only when the compat shim synthesized a legacy tokio-era report.
export const isAsyncRuntimeBuild: boolean = capabilities.asyncRuntimeBuild;
