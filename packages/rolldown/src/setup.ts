import { isMainThread } from 'node:worker_threads';
import { onExit } from 'signal-exit';
import { initTraceSubscriber } from './binding';

if (isMainThread) {
  const subscriberGuard = initTraceSubscriber();
  onExit(() => {
    subscriberGuard?.close();
  });
}
