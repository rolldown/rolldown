import { build } from 'rolldown';
import { expect, test } from 'vitest';

test.each(['\0preload', 'entry'])('this.load returns ModuleInfo in buildStart: %j', async (id) => {
  await build({
    input: 'entry',
    write: false,
    plugins: [
      {
        name: 'preload',
        async buildStart() {
          const info = await this.load({ id });
          expect(info).toMatchObject({ id, code: 'export default 1', isEntry: false });
        },
        buildEnd() {
          expect(this.getModuleInfo('entry')?.isEntry).toBe(true);
        },
        resolveId: (id) => id,
        load: () => 'export default 1',
      },
    ],
  });
});

test('entry and emitted chunk loading waits for every buildStart', async () => {
  let buildStartFinished = false;
  const loaded: string[] = [];
  await build({
    input: 'entry',
    write: false,
    plugins: [
      {
        name: 'preload',
        async buildStart() {
          this.emitFile({ type: 'chunk', id: 'emitted' });
          await this.load({ id: '\0preload' });
        },
        resolveId(id) {
          expect(buildStartFinished).toBe(true);
          return id;
        },
        load(id) {
          loaded.push(id);
          expect(buildStartFinished).toBe(id !== '\0preload');
          return 'export default 1';
        },
        transform(_code, id) {
          if (id === 'entry' || id === 'emitted') {
            expect(buildStartFinished).toBe(true);
          }
        },
        moduleParsed(info) {
          if (info.id === 'entry' || info.id === 'emitted') {
            expect(buildStartFinished).toBe(true);
          }
        },
      },
      {
        name: 'last-build-start',
        async buildStart() {
          await Promise.resolve();
          buildStartFinished = true;
        },
      },
    ],
  });
  expect(loaded.toSorted()).toEqual(['\0preload', 'emitted', 'entry']);
});

test('a preloaded entry with a dynamic self-import produces one entry chunk', async () => {
  let dependencyLoaded!: () => void;
  const loaded = new Promise<void>((resolve) => (dependencyLoaded = resolve));
  const { output } = await build({
    input: { named: 'entry' },
    write: false,
    plugins: [
      {
        name: 'preload',
        async buildStart() {
          await this.load({ id: 'entry' });
          await loaded;
        },
        resolveId: (id) => id,
        load(id) {
          if (id === 'dependency') {
            dependencyLoaded();
            return '';
          }
          return 'import "dependency"; export const self = () => import("entry"); export default 1';
        },
      },
    ],
  });
  expect(output).toHaveLength(1);
  expect(output[0]).toMatchObject({ fileName: 'named.js', isEntry: true, facadeModuleId: 'entry' });
});

test('this.load rejects when loading a module fails', async () => {
  let loadError: unknown;
  await expect(
    build({
      input: 'entry',
      write: false,
      plugins: [
        {
          name: 'preload',
          async buildStart() {
            try {
              await this.load({ id: '\0preload' });
            } catch (error) {
              loadError = error;
              throw error;
            }
          },
          resolveId: (id) => id,
          load(id) {
            if (id === '\0preload') throw new Error('preload failed');
            return 'export default 1';
          },
        },
      ],
    }),
  ).rejects.toThrow('preload failed');
  expect(loadError).toMatchObject({ message: expect.stringContaining('preload failed') });
});

test('this.load rejects outside the scan hooks', async () => {
  await build({
    input: 'entry',
    write: false,
    plugins: [
      {
        name: 'preload',
        resolveId: (id) => id,
        load: () => 'export default 1',
        async renderStart() {
          await expect(this.load({ id: '\0after-scan' })).rejects.toThrow('PluginContext.load');
        },
      },
    ],
  });
});
