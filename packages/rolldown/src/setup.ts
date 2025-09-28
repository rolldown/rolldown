import { isMainThread } from 'node:worker_threads';
import { initTraceSubscriber } from './binding';

if (!import.meta.browserBuild && isMainThread) {
  const subscriberGuard = initTraceSubscriber();
  if (subscriberGuard) {
    process.on('exit', () => subscriberGuard.close());
  }
}
