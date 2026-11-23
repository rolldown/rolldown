import { rolldown } from 'rolldown';
import { getOutputChunk } from 'rolldown-tests/utils';
import { expect, test } from 'vitest';

test.each([
  { a: '[hash].js', b: '[hash].js' },
  { a: 'leaf/./[hash].js', b: 'leaf/[hash].js' },
  { a: 'tmp/../leaf-[hash].js', b: 'leaf-[hash].js' },
  { a: 'leaf//[hash].js', b: 'leaf/[hash].js' },
])('changes an importer hash when a child reorders $a and $b', async ({ a, b }) => {
  async function build(dependencies: string[]) {
    const modules: Record<string, string> = {
      'main.js': 'import "./b.js"; import "./child.js"; console.log("main");',
      'child.js':
        dependencies.map((name) => `import "./${name}.js";`).join('\n') + '\nconsole.log("child");',
      'a.js': 'console.log("a");',
      'b.js': 'console.log("b");',
      'only-a.js': 'import "./a.js"; console.log("only-a");',
      'only-b.js': 'import "./b.js"; console.log("only-b");',
      'only-child.js': 'import "./child.js"; console.log("only-child");',
    };
    const bundle = await rolldown({
      input: ['main.js', 'only-a.js', 'only-b.js', 'only-child.js'],
      plugins: [
        {
          name: 'virtual',
          resolveId(id) {
            const file = id.replace(/^\.\//, '');
            if (Object.hasOwn(modules, file)) return file;
          },
          load(id) {
            return modules[id];
          },
        },
      ],
    });
    try {
      const output = await bundle.generate({
        entryFileNames: '[name]-[hash].js',
        chunkFileNames: ({ name }) => {
          if (name === 'a') return a;
          if (name === 'b') return b;
          return '[hash].js';
        },
      });
      const entry = getOutputChunk(output).find((chunk) => chunk.name === 'main');
      expect(entry).toBeDefined();
      return entry!;
    } finally {
      await bundle.close();
    }
  }

  const before = await build(['a', 'b']);
  const after = await build(['b', 'a']);
  expect(after.code).not.toBe(before.code);
  expect(after.fileName).not.toBe(before.fileName);
});
