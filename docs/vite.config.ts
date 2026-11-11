// The docs tasks are defined here so `vp run` caches them.
export default {
  run: {
    tasks: {
      generate: 'node .vitepress/scripts/generate-reference.ts',
      build: 'vitepress build',
    },
  },
};
