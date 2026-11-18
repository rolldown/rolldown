import path from 'node:path';
import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

const refs: Record<string, string> = {};

export default defineTest({
  config: {
    input: 'main.js',
    plugins: [
      {
        name: 'test-plugin',
        buildStart() {
          for (const dir of ['a', 'b']) {
            refs[`./w.js from ${dir}`] = this.emitFile({
              type: 'chunk',
              id: './w.js',
              importer: path.join(import.meta.dirname, dir, 'index.js'),
            });
            refs[`./${dir}/n.js named worker`] = this.emitFile({
              type: 'chunk',
              id: `./${dir}/n.js`,
              name: 'worker',
            });
          }
          for (const fileName of ['x1.js', 'x2.js']) {
            refs[`./f.js as ${fileName}`] = this.emitFile({
              type: 'chunk',
              id: './f.js',
              fileName,
            });
          }
        },
        generateBundle(_, bundle) {
          const resolved = Object.fromEntries(
            Object.entries(refs).map(([key, ref]) => {
              const fileName = this.getFileName(ref);
              const chunk = bundle[fileName];
              const facade = chunk.type === 'chunk' ? chunk.facadeModuleId : null;
              return [
                key,
                facade && path.relative(import.meta.dirname, facade).replaceAll('\\', '/'),
              ];
            }),
          );
          expect(resolved).toEqual({
            './w.js from a': 'a/w.js',
            './w.js from b': 'b/w.js',
            './a/n.js named worker': 'a/n.js',
            './b/n.js named worker': 'b/n.js',
            './f.js as x1.js': 'f.js',
            './f.js as x2.js': 'f.js',
          });
          expect(this.getFileName(refs['./f.js as x1.js'])).toBe('x1.js');
          expect(this.getFileName(refs['./f.js as x2.js'])).toBe('x2.js');
        },
      },
    ],
  },
});
