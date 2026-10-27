import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';

import type { OutputBundle, OutputChunk, Plugin } from 'rolldown';
import { rolldown } from 'rolldown';
import { expect, test } from 'vitest';

// Plugins that read `chunk.code` in the output hooks share one JavaScript string per chunk.
// These tests check that the shared string never hides a newer `code`.

function virtualPlugin(modules: Record<string, string>): Plugin {
  return {
    name: 'virtual',
    resolveId(id) {
      if (id in modules) return id;
    },
    load(id) {
      return modules[id];
    },
  };
}

function chunksOf(bundle: OutputBundle): OutputChunk[] {
  return Object.values(bundle).filter((item): item is OutputChunk => item.type === 'chunk');
}

const MARKER = '// changed by writer\n';

test('a plugin reads the code and file name that an earlier plugin assigned', async () => {
  const seen: string[] = [];
  const record = (hook: string, bundle: OutputBundle) => {
    for (const chunk of chunksOf(bundle)) {
      seen.push(`${hook}:${chunk.fileName}:${chunk.code.endsWith(MARKER)}`);
    }
  };
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'rolldown-output-code-cache-'));
  const bundle = await rolldown({
    input: 'entry',
    plugins: [
      virtualPlugin({ entry: 'console.log("entry");' }),
      {
        name: 'reader-before',
        generateBundle(_options, bundle) {
          record('before', bundle);
        },
      },
      {
        name: 'writer-code',
        generateBundle(_options, bundle) {
          for (const chunk of chunksOf(bundle)) {
            chunk.code = chunk.code + MARKER;
          }
        },
      },
      {
        // Same file name as the cached string, so this read must notice the new code.
        name: 'reader-after-code',
        generateBundle(_options, bundle) {
          record('afterCode', bundle);
        },
      },
      {
        name: 'writer-rename',
        generateBundle(_options, bundle) {
          for (const chunk of chunksOf(bundle)) {
            chunk.fileName = 'renamed.js';
          }
        },
      },
      {
        name: 'reader-after-rename',
        generateBundle(_options, bundle) {
          record('afterRename', bundle);
        },
        writeBundle(_options, bundle) {
          record('writeBundle', bundle);
        },
      },
    ],
  });
  try {
    const { output } = await bundle.write({ dir });
    expect(seen).toStrictEqual([
      'before:entry.js:false',
      'afterCode:entry.js:true',
      'afterRename:renamed.js:true',
      'writeBundle:renamed.js:true',
    ]);
    expect(output.map((item) => item.fileName)).toStrictEqual(['renamed.js']);
    expect((output[0] as OutputChunk).code.endsWith(MARKER)).toBe(true);
    expect(fs.readFileSync(path.join(dir, 'renamed.js'), 'utf8').endsWith(MARKER)).toBe(true);
  } finally {
    await bundle.close();
    fs.rmSync(dir, { recursive: true, force: true });
  }
});

test('a second build on the same bundler reads the new code', async () => {
  let build = 0;
  const seen: string[] = [];
  const bundle = await rolldown({
    input: 'entry',
    plugins: [
      virtualPlugin({ entry: 'console.log("entry");' }),
      {
        name: 'stamp',
        renderChunk(code) {
          return `${code}// build ${build}\n`;
        },
      },
      {
        name: 'reader',
        generateBundle(_options, bundle) {
          for (const chunk of chunksOf(bundle)) {
            seen.push(chunk.code.trimEnd().split('\n').at(-1)!);
          }
        },
      },
    ],
  });
  try {
    build = 1;
    await bundle.generate({});
    build = 2;
    await bundle.generate({});
    expect(seen).toStrictEqual(['// build 1', '// build 2']);
  } finally {
    await bundle.close();
  }
});
