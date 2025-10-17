import { defineDevConfig } from '@rolldown/test-dev-server';

export default defineDevConfig({
  platform: 'node',
  build: {
    input: 'src/main.js',
    experimental: {
      hmr: {},
    },
    platform: 'node',
    treeshake: false,
    plugins: [
      {
        name: 'update-module-sideeffects',
        async transform(_code, id) {
          if (id.endsWith('main.js')) {
            const resolved = await this.resolve('./foo.js', id);
            if (!resolved) throw new Error('Could not resolve foo.js');
            const moduleInfo = this.getModuleInfo(resolved.id);
            if (moduleInfo) {
              moduleInfo.moduleSideEffects = true;
            } else if (!resolved.external) {
              await this.load({
                ...resolved,
                moduleSideEffects: true,
              });
            }
          }
        },
      },
    ],
  },
});
