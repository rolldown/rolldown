import { defineConfig } from 'rolldown';
import MagicString from 'magic-string';

export default defineConfig({
  input: ['/virtual'],
  output: {
    dir: 'dist',
    sourcemap: true,
  },
  plugins: [
    {
      name: 'test',
      resolveId(id) {
        if (id === '/virtual') {
          return '/virtual';
        }
      },
      load(code, id) {
        const s = new MagicString('export const foo = 42');
        s.prepend('//hello\n');
let map = s.generateMap({
            hires: true,
            includeContent: false,
            source: id,
          });
        console.log(`map.sourcesContent: `, map.sourcesContent)
        return {
          code: s.toString(),
          map,
        };
      },
    },
  ],
});

