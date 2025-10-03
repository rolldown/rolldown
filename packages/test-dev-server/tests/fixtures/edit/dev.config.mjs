import { defineDevConfig } from '@rolldown/test-dev-server';

let lastWatchChangeResult = '';

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
        name: 'watch-change-result',
        watchChange(_id, _kind) {
          lastWatchChangeResult = `called`;
        },
        transform(code, id) {
          if (id.endsWith('foo.js')) {
            return code.replace(
              '__WATCH_CHANGE_RESULT__',
              JSON.stringify('-' + lastWatchChangeResult),
            );
          }
        },
      },
    ],
  },
});
