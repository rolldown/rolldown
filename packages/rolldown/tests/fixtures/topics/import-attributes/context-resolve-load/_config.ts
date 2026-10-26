import path from 'node:path';
import { defineTest } from 'rolldown-tests';
import { stringifyModuleId } from 'rolldown/utils';
import { expect } from 'vitest';

const main = path.join(import.meta.dirname, 'main.js');
const data = path.join(import.meta.dirname, 'data.js');
const seenByResolveId: unknown[] = [];

export default defineTest({
  config: {
    input: main,
    experimental: { importAttributes: true },
    plugins: [
      {
        name: 'test-plugin',
        resolveId(source, importer, options) {
          if (source === './data.js' && importer !== main) {
            seenByResolveId.push({
              importer,
              importerRawId: options.importerRawId,
              importerAttributes: options.importerAttributes,
            });
          }
        },
        async transform(_code, id) {
          if (id !== main) return;
          const resolved = await this.resolve('./data.js', id, { attributes: { type: 'foo' } });
          expect(resolved).toMatchObject({
            id: data,
            attributes: { type: 'foo' },
            external: false,
          });

          // Like in Rollup 5, `this.load` takes the attributes only from `id`.
          const info = await this.load({
            ...resolved!,
            id: { rawId: resolved!.id, attributes: resolved!.attributes },
          });
          expect(info.id).toBe(stringifyModuleId(data, { type: 'foo' }));
          // Passed as is, the `this.resolve` result loads the module without attributes.
          expect((await this.load(resolved!)).id).toBe(data);
          const barId = stringifyModuleId(data, { type: 'bar' });
          expect((await this.load({ id: barId })).id).toBe(barId);

          // A `null` importer is accepted, like in Rollup.
          expect(await this.resolve('unknown-module', null as unknown as undefined)).toBeNull();

          // A `{ rawId, attributes }` importer reaches `resolveId` as the module id.
          await this.resolve(
            './data.js',
            { rawId: main, attributes: { kind: 'x' } },
            { skipSelf: false },
          );
          await this.resolve('./data.js', stringifyModuleId(main, { kind: 'y' }), {
            skipSelf: false,
          });
        },
      },
    ],
  },
  afterTest() {
    expect(seenByResolveId).toEqual([
      {
        importer: stringifyModuleId(main, { kind: 'x' }),
        importerRawId: main,
        importerAttributes: { kind: 'x' },
      },
      {
        importer: stringifyModuleId(main, { kind: 'y' }),
        importerRawId: main,
        importerAttributes: { kind: 'y' },
      },
    ]);
  },
});
