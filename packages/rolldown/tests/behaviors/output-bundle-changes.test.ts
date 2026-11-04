import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';

import type { OutputBundle, OutputChunk, Plugin } from 'rolldown';
import { rolldown } from 'rolldown';
import { expect, test } from 'vitest';

// A `generateBundle` hook that assigns a property of a chunk sends the chunk back to Rust. Only
// the properties the plugin read or assigned are sent, and none if they still match Rust's values.
// These tests check that the next hook and the written output still see every change, and only
// those changes.

function virtualPlugin(modules: Record<string, string>): Plugin {
  return {
    name: 'virtual',
    resolveId(id) {
      if (id in modules) return id;
    },
    load(id) {
      if (id.endsWith('.css')) {
        // Like a stylesheet in Vite: an empty module that is never tree-shaken.
        return { code: '', moduleType: 'js', moduleSideEffects: 'no-treeshake' };
      }
      return modules[id];
    },
  };
}

const modules = {
  entry: `import 'style.css';\nimport { shared } from 'shared';\nconsole.log(shared);\nexport const lazy = () => import('lazy');\n`,
  lazy: `import 'style.css';\nimport { shared } from 'shared';\nexport default shared + 1;\n`,
  shared: `export const shared = 'shared';\n`,
  'style.css': '',
};

function chunksOf(bundle: OutputBundle): OutputChunk[] {
  return Object.values(bundle).filter((item): item is OutputChunk => item.type === 'chunk');
}

interface ChunkView {
  code: string;
  map: string | null;
  imports: string[];
  dynamicImports: string[];
}

function record(bundle: OutputBundle): Record<string, ChunkView> {
  return Object.fromEntries(
    chunksOf(bundle).map((chunk) => [
      chunk.fileName,
      {
        code: chunk.code,
        map: chunk.map ? chunk.map.toString() : null,
        imports: chunk.imports,
        dynamicImports: chunk.dynamicImports,
      },
    ]),
  );
}

// What Vite's `vite:css-post` does in `generateBundle` when a build has a pure CSS chunk: it
// assigns `imports` of every chunk, rewrites the import of the empty chunk out of the code of the
// chunks that had it (keeping the length, so their maps stay right), and deletes the empty chunk.
const cssPost: Plugin = {
  name: 'css-post',
  generateBundle(_options, bundle) {
    const pureCss = chunksOf(bundle)
      .filter((chunk) => chunk.moduleIds.every((id) => id.endsWith('.css')))
      .map((chunk) => chunk.fileName);
    expect(pureCss).toStrictEqual(['style.js']);
    for (const chunk of chunksOf(bundle)) {
      let importsPureCss = false;
      chunk.imports = chunk.imports.filter((fileName) => {
        if (!pureCss.includes(fileName)) return true;
        importsPureCss = true;
        return false;
      });
      if (importsPureCss) {
        chunk.code = chunk.code.replace(
          /\bimport\s*["'][^"']*style\.js["'];/g,
          (m) => `/* empty css ${''.padEnd(m.length - 15)}*/`,
        );
      }
    }
    for (const fileName of pureCss) {
      delete bundle[fileName];
      delete bundle[`${fileName}.map`];
    }
  },
};

test('assigning `imports` keeps the code and map of a chunk unless the plugin changed them', async () => {
  let before: Record<string, ChunkView> = {};
  let after: Record<string, ChunkView> = {};
  let written: Record<string, ChunkView> = {};
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'rolldown-output-bundle-changes-'));
  const bundle = await rolldown({
    input: 'entry',
    plugins: [
      virtualPlugin(modules),
      {
        name: 'reader-before',
        generateBundle(_options, bundle) {
          before = record(bundle);
        },
      },
      cssPost,
      {
        name: 'reader-after',
        generateBundle(_options, bundle) {
          after = record(bundle);
        },
        writeBundle(_options, bundle) {
          written = record(bundle);
        },
      },
    ],
  });
  try {
    const { output } = await bundle.write({
      dir,
      sourcemap: true,
      chunkFileNames: '[name].js',
      codeSplitting: {
        groups: [
          { name: 'style', test: /\.css$/ },
          { name: 'shared', test: /^shared$/ },
        ],
      },
    });

    expect(Object.keys(before).sort()).toStrictEqual([
      'entry.js',
      'lazy.js',
      'shared.js',
      'style.js',
    ]);
    expect(before['entry.js'].imports).toStrictEqual(['style.js', 'shared.js']);
    expect(before['lazy.js'].imports).toStrictEqual(['style.js', 'shared.js']);
    expect(before['entry.js'].code).toMatch(/import\s*["']\.\/style\.js["'];/);

    // The deleted chunk stays deleted, in later hooks and on disk.
    expect(Object.keys(after).sort()).toStrictEqual(['entry.js', 'lazy.js', 'shared.js']);
    expect(written).toStrictEqual(after);
    expect(fs.existsSync(path.join(dir, 'style.js'))).toBe(false);
    expect(fs.existsSync(path.join(dir, 'style.js.map'))).toBe(false);

    for (const fileName of ['entry.js', 'lazy.js']) {
      // The assigned `imports` and `code` reach the next hook...
      expect(after[fileName].imports).toStrictEqual(['shared.js']);
      expect(after[fileName].code).not.toContain('style.js');
      expect(after[fileName].code).toContain('/* empty css');
      expect(after[fileName].code.length).toBe(before[fileName].code.length);
      // ...and the map that no plugin read or assigned is the one Rust had.
      expect(after[fileName].map).toBe(before[fileName].map);
      expect(after[fileName].dynamicImports).toStrictEqual(before[fileName].dynamicImports);
    }
    // `shared.js` was assigned the same `imports`, so nothing about it changed.
    expect(after['shared.js']).toStrictEqual(before['shared.js']);

    // The written files and the returned output have the same code and maps.
    for (const [fileName, view] of Object.entries(after)) {
      expect(fs.readFileSync(path.join(dir, fileName), 'utf8')).toBe(view.code);
      expect(JSON.parse(fs.readFileSync(path.join(dir, `${fileName}.map`), 'utf8'))).toStrictEqual(
        JSON.parse(before[fileName].map!),
      );
      const chunk = output.find((item) => item.fileName === fileName) as OutputChunk;
      expect(chunk.code).toBe(view.code);
      expect(chunk.map!.toString()).toBe(view.map);
      expect(chunk.imports).toStrictEqual(view.imports);
    }
    expect(output.map((item) => item.fileName).sort()).toStrictEqual([
      'entry.js',
      'entry.js.map',
      'lazy.js',
      'lazy.js.map',
      'shared.js',
      'shared.js.map',
    ]);
  } finally {
    await bundle.close();
    fs.rmSync(dir, { recursive: true, force: true });
  }
});

test('changes made in place to a read `imports` or `map` reach Rust with an assigned chunk', async () => {
  let before: Record<string, ChunkView> = {};
  let after: Record<string, ChunkView> = {};
  const bundle = await rolldown({
    input: 'entry',
    plugins: [
      virtualPlugin(modules),
      {
        name: 'reader-before',
        generateBundle(_options, bundle) {
          before = record(bundle);
        },
      },
      {
        name: 'writer',
        generateBundle(_options, bundle) {
          const entry = bundle['entry.js'] as OutputChunk;
          entry.imports.push('extra.js');
          entry.map!.names.push('inPlaceName');
          entry.code += '// changed\n';

          const lazy = bundle['lazy.js'] as OutputChunk;
          lazy.map = null;

          const shared = bundle['shared.js'] as OutputChunk;
          // Assigned the values it had.
          shared.dynamicImports = [...shared.dynamicImports];
          const { isEntry } = shared;
          shared.isEntry = isEntry;
        },
      },
      {
        name: 'reader-after',
        generateBundle(_options, bundle) {
          after = record(bundle);
        },
      },
    ],
  });
  try {
    const { output } = await bundle.generate({
      sourcemap: true,
      chunkFileNames: '[name].js',
      codeSplitting: { groups: [{ name: 'shared', test: /^shared$/ }] },
    });

    expect(after['entry.js'].imports).toStrictEqual([...before['entry.js'].imports, 'extra.js']);
    expect(after['entry.js'].code).toBe(`${before['entry.js'].code}// changed\n`);
    expect(JSON.parse(after['entry.js'].map!).names).toStrictEqual([
      ...JSON.parse(before['entry.js'].map!).names,
      'inPlaceName',
    ]);
    expect(after['lazy.js'].map).toBeNull();
    expect(after['lazy.js'].code).toBe(before['lazy.js'].code);
    expect(after['shared.js']).toStrictEqual(before['shared.js']);

    const outputChunk = (fileName: string) =>
      output.find((item) => item.fileName === fileName) as OutputChunk;
    expect(outputChunk('entry.js').imports).toStrictEqual(after['entry.js'].imports);
    expect(outputChunk('entry.js').code).toBe(after['entry.js'].code);
    expect(outputChunk('lazy.js').map).toBeNull();
    expect(outputChunk('shared.js').map!.toString()).toBe(before['shared.js'].map);
  } finally {
    await bundle.close();
  }
});
