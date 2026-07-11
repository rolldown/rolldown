import { defineParallelPluginImplementation } from 'rolldown/parallelPlugin';
import { createSvelteRegistryGraphPlugin } from './kernel.js';

export default defineParallelPluginImplementation((options, context) =>
  createSvelteRegistryGraphPlugin(options, context.threadNumber),
);
