import type { OutputChunk } from 'rolldown';
import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

const referenceIds: string[] = [];
const codeSplitting = {
  includeDependenciesRecursively: false,
  groups: [{ name: 'group', test: /[\\/][ab]\.js$/ }],
};
const fileNamesMarker = '// file names of the emitted chunks: ';

export default defineTest({
  config: {
    input: 'main.js',
    preserveEntrySignatures: 'allow-extension',
    output: [
      { format: 'es', codeSplitting },
      { format: 'cjs', codeSplitting },
    ],
    plugins: [
      {
        name: 'test-plugin',
        buildStart() {
          referenceIds.length = 0;
          for (const id of ['./a.js', './b.js']) {
            referenceIds.push(
              this.emitFile({ type: 'chunk', id, preserveSignature: 'allow-extension' }),
            );
          }
        },
        renderChunk(code, chunk) {
          if (chunk.name !== 'main') {
            return null;
          }
          // In `renderChunk`, the `group` filename from `getFileName()` contains a hash placeholder.
          // Rolldown replaces the placeholder in the output code.
          const fileNames = referenceIds.map((referenceId) => this.getFileName(referenceId));
          return `${code}\n${fileNamesMarker}${JSON.stringify(fileNames)}\n`;
        },
        generateBundle(_, bundle) {
          const findChunk = (name: string) =>
            Object.values(bundle).find(
              (output): output is OutputChunk => output.type === 'chunk' && output.name === name,
            )!;
          // The `group` chunk contains both entry modules. Each module has only local exports.
          // The chunk optimizer therefore removes both facades.
          // The `group` chunk provides the exports of both entries.
          const group = findChunk('group');
          expect(group.exports).toEqual(['a', 'b']);
          expect(referenceIds.map((referenceId) => this.getFileName(referenceId))).toEqual([
            group.fileName,
            group.fileName,
          ]);
          const fileNamesLine = findChunk('main')
            .code.split('\n')
            .find((line) => line.startsWith(fileNamesMarker));
          expect(JSON.parse(fileNamesLine!.slice(fileNamesMarker.length))).toEqual([
            group.fileName,
            group.fileName,
          ]);
        },
      },
    ],
  },
});
