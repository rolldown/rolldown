export const log = [];

using syncResource = {
  kind: 'sync',
  [Symbol.dispose]() {
    log.push('sync disposed');
  },
};

await using asyncResource = {
  kind: 'async',
  async [Symbol.asyncDispose]() {
    await Promise.resolve();
    log.push('async disposed');
  },
};

export const getSync = () => syncResource;
export const getAsync = () => asyncResource;
