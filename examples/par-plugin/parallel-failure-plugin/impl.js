import { defineParallelPluginImplementation } from 'rolldown/parallelPlugin';

export default defineParallelPluginImplementation((options) => {
  if (options.mode === 'init') {
    throw new Error('parallel-plugin-init-failure');
  }

  return {
    name: `parallel-failure-${options.mode}`,
    transform(_code, id) {
      if (!id.endsWith('/input.js')) {
        return;
      }
      if (options.mode === 'sync') {
        throw new Error('parallel-plugin-sync-transform-failure');
      }
      return Promise.reject(new Error('parallel-plugin-rejected-transform-failure'));
    },
  };
});
