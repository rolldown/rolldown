import { getRuntimeSupport } from 'rolldown/experimental';

const support = getRuntimeSupport();

// Everything runs on the calling thread: `dev()` needs MultiThread, so it is
// unsupported exactly on the CurrentThread flavor.
export const isSingleThread: boolean = !support.dev;

// Any WASI artifact (`watch` is false exactly there). Watch and symlink
// traversal are off on every WASI artifact, even threaded MultiThread, so gate
// those on this.
export const isWasiTest: boolean = !support.watch;

// Threadless WASI only. Same field `src/utils/threadless-free.ts` reads, so
// eager-free assertions match the package.
export const isThreadlessWasi: boolean = support.threadlessWasi;
