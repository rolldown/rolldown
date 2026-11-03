import { isMainThread } from 'node:worker_threads';
import { initTraceSubscriber } from './binding.cjs';
import { onExit } from './utils/signal-exit';

if (!import.meta.browserBuild && isMainThread) {
  const subscriberGuard = initTraceSubscriber();
  // The guard only exists when a trace output that needs flushing is active (chrome tracing).
  // `signal-exit` installs SIGHUP/SIGINT/SIGTERM listeners and re-raises the signal, which
  // keeps the process alive while a debugger is attached (`node --inspect`), so don't install
  // it merely because rolldown was imported.
  if (subscriberGuard) {
    onExit(() => {
      subscriberGuard.close();
    });
  }
}
