import { getAsyncRuntimeConfig } from 'rolldown/experimental';

// Flavor is reported by every build: async-runtime builds report the configured
// executor; the default tokio build reports its init-time snapshot
// ('MultiThread').
export const runtimeFlavor: string = getAsyncRuntimeConfig().flavor;

// True when the binding schedules everything on the calling thread
// (native async-runtime build with ROLLDOWN_RUNTIME=single, or the wasip1
// build).
export const isSingleThread: boolean = runtimeFlavor === 'CurrentThread';

// Set by the WASI CI lane (reusable-wasi.yml) — distinguishes "wasm binding"
// from "native binding in single-thread mode".
export const isWasiTest: boolean = process.env.ROLLDOWN_TEST_WASI === '1';
