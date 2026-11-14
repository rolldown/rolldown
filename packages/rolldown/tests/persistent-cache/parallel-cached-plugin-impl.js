import fs from 'node:fs';
import { defineParallelPluginImplementation } from 'rolldown/parallelPlugin';

// After setup, nothing keeps the event loop of a worker alive, so the worker exits and each hook
// call fails with `Closing`. This timer keeps the worker alive until `bundle.close()` stops it.
// Remove it when the worker stays alive by itself.
setInterval(() => {}, 1 << 30);

/**
 * Appends the id of each module in `options.src` that its `transform` hook runs for to the file
 * `options.log`. The hook runs in a worker, so a file is how the test sees the calls.
 */
export default defineParallelPluginImplementation((options) => ({
  name: 'parallel-cached',
  transform: {
    cache: options.cache,
    filter: options.only ? { id: new RegExp(options.only) } : undefined,
    handler(_code, id) {
      if (id.startsWith(options.src)) {
        fs.appendFileSync(options.log, `${id}\n`);
      }
      return null;
    },
  },
}));
