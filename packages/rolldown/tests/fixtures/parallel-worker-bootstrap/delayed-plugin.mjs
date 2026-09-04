import { defineParallelPluginImplementation } from 'rolldown/parallelPlugin';

export default defineParallelPluginImplementation(async ({ delay }) => {
  await new Promise((resolve) => setTimeout(resolve, delay));
  return {
    name: 'delayed-parallel-worker-bootstrap',
  };
});
