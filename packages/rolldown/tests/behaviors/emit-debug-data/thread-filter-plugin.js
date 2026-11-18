import { defineParallelPluginImplementation } from 'rolldown/parallelPlugin';

// After setup, nothing keeps the event loop of a worker alive, so the worker exits and each hook
// call fails with `Closing`. This timer keeps the worker alive until `bundle.close()` stops it.
// Remove it when the worker stays alive by itself.
setInterval(() => {}, 1 << 30);

// Each worker accepts only the module named after its thread number.
export default defineParallelPluginImplementation((_options, { threadNumber }) => ({
  name: 'thread-filter',
  transform: {
    filter: { id: new RegExp(`^m${threadNumber}$`) },
    handler: (code) => code,
  },
}));
