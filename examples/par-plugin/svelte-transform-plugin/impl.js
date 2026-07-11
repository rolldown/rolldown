import { defineParallelPluginImplementation } from 'rolldown/parallelPlugin';
import { createSvelteTransformPlugin } from './kernel.js';

export default defineParallelPluginImplementation((options, context) =>
  createSvelteTransformPlugin(options, context.threadNumber),
);
