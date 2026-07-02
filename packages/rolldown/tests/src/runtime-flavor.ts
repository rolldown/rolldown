import { getRuntimeCapabilities } from 'rolldown/experimental';

// The binding self-reports what it IS (backend / flavor / target) through
// `getRuntimeCapabilities()`: compile-time facts plus the config snapshot
// resolved once at addon load. The helpers below are capability queries
// against the loaded artifact -- nothing here reads lane env vars or probes
// error messages. Querying at module scope is safe in every lane: this module
// already loaded the binding at collection time for `runtimeFlavor` before.
const capabilities = getRuntimeCapabilities();

// Flavor is reported by every build: async-runtime builds report the
// configured executor; the default tokio build reports 'MultiThread'.
export const runtimeFlavor: string = capabilities.flavor;

// True when the binding schedules everything on the calling thread
// (native async-runtime build with ROLLDOWN_RUNTIME=single, or the wasip1
// build). Equivalent to `capabilities.flavor === 'CurrentThread'`.
export const isSingleThread: boolean = !capabilities.threads;

// True when the loaded binding is a WebAssembly/WASI artifact ('wasi' or
// 'wasi-threads' target) -- distinguishes "wasm binding" from "native binding
// in single-thread mode". Gates the wasm-boundary-specific skips (error DX,
// watch, symlinks); no CI env var involved, the artifact identifies itself.
export const isWasiTest: boolean = capabilities.wasi;

// True when the binding was compiled with `--features async-runtime` (either
// flavor).
export const isAsyncRuntimeBuild: boolean = capabilities.asyncRuntimeBuild;
