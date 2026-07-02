import { isMainThread } from 'node:worker_threads';
import { registerTimerHost } from './binding.cjs';

// Timer host for the `--features async-runtime` binding: its CurrentThread
// flavor delegates timers (e.g. the watch-mode debounce) to the host event
// loop. A no-op on the default tokio build.
//
// This lives in its own side-effect module because both the library entry
// (via `setup.ts`) and the CLI entry need it: the CLI bundle does not include
// `setup.ts`, and watch mode must have a driver registered before the first
// debounce timer arms.
if (!import.meta.browserBuild && isMainThread) {
  registerTimerHost((ms) => new Promise((resolve) => setTimeout(resolve, ms)));
}
