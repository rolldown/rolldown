import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';
import { globSync } from 'glob';
import path from 'node:path';

export default defineTest({
  sequential: true,
  config: {
    input: ['src/main/index.js', 'src/bar/bar.js', 'src/foo/foo.js'],
    output: {
      preserveModules: true,
      preserveModulesRoot: 'src/main',
    },
  },
  afterTest: () => {
    let files = globSync('**/*.js', { cwd: path.resolve(import.meta.dirname, './dist') });
    files.sort();
    files = files.map((file) => file.replace(/\\/g, '/'));
    expect(files).toMatchInlineSnapshot(`
      [
        "bar/bar.js",
        "foo/foo.js",
        "index.js",
      ]
    `);
  },
});
