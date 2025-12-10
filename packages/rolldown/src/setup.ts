import { isMainThread } from 'node:worker_threads';
import { initTraceSubscriber } from './binding.cjs';
import { onExit } from './utils/signal-exit';

if (!import.meta.browserBuild && isMainThread) {
  const subscriberGuard = initTraceSubscriber();
  onExit(() => {
    subscriberGuard?.close();
  });
}
