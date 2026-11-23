import path from 'node:path';
import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

const main = path.join(import.meta.dirname, 'main.js');
const worker = path.join(import.meta.dirname, 'worker.js');
const probe = path.join(import.meta.dirname, 'probe.js');
let markMainParsed!: () => void;
const mainParsed = new Promise<void>((resolve) => {
  markMainParsed = resolve;
});

export default defineTest({
  config: {
    input: { main, worker },
    experimental: { resolveNewUrlToAsset: true },
    plugins: [
      {
        name: 'resolve-worker-entry-late',
        async resolveId(source, _importer, options) {
          if (!options.isEntry || source !== worker) return;
          await mainParsed;
          // The module loader handles messages in order. `this.load` sends one after `main.js` is
          // parsed, so `main.js` is processed by the time it returns.
          await this.load({ id: probe });
        },
        moduleParsed(info) {
          if (info.id === main) markMainParsed();
        },
      },
    ],
  },
  afterTest(output) {
    // The worker is an entry chunk, not an asset with a CommonJS wrapper chunk.
    const files = output.output;
    expect(files.map((file) => file.type)).toStrictEqual(['chunk', 'chunk']);
    const mainChunk = files.find((file) => file.fileName === 'main.js') as { code: string };
    const workerChunk = files.find((file) => file.fileName !== 'main.js') as {
      fileName: string;
      isEntry: boolean;
      code: string;
    };
    expect(workerChunk.isEntry).toBe(true);
    expect(workerChunk.code).toContain('workerExecuted');
    expect(mainChunk.code).toContain(`new URL("./worker.js", import.meta.url)`);
  },
});
