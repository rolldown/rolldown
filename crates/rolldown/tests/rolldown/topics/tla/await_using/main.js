import { log } from './log.js';
{
  await using _ = {
    async [Symbol.asyncDispose]() {
      await Promise.resolve();
      log.push('disposed');
    },
  };
  log.push('body');
}
export { log };
