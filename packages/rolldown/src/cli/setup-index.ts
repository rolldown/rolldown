// Initialize Tokio runtime as early as possible to avoid wasting time on imports

import { createTokioRuntime } from '../binding.cjs';

let isWatchMode = false;

// Check for --watch or -w flag directly in process.argv. This is a bit hacky but good enough.
for (let i = 0; i < process.argv.length; i++) {
  const arg = process.argv[i];
  if (arg === '--watch' || arg === '-w') {
    isWatchMode = true;
    break;
  }
}

if (isWatchMode) {
  // Due to implementation of watch, if we don't use this much of threads, it'll hang.
  createTokioRuntime(32);
} else {
  createTokioRuntime(4);
}
