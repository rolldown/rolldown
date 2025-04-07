import { defineParallelPluginImplementation } from 'rolldown/parallel-plugin';
/** @returns {import('rolldown').Plugin} */
export const noopPlugin = () => {
  return {
    name: 'noop',
    transform(_code, _id) {
      const now = performance.now();
      while (performance.now() - now < 1) {}
    },
  };
};

export default defineParallelPluginImplementation((_options, _context) => {
  return noopPlugin();
});
