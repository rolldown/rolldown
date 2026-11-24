import { log } from './log.js';

using syncResource = {
  [Symbol.dispose]() {
    log.push('sync disposed');
  },
};

await using asyncResource = {
  async [Symbol.asyncDispose]() {
    await Promise.resolve();
    log.push('async disposed');
  },
};

log.push('body', typeof syncResource, typeof asyncResource);

export { log };
