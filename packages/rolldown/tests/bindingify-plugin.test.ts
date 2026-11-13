import { describe, expect, test, vi } from 'vitest';
import { bindingifyPlugin } from '../src/plugin/bindingify-plugin';
import { PluginContextData } from '../src/plugin/plugin-context-data';
import { pluginTimingsRecorderFor } from '../src/utils/plugin-timings';

vi.mock('../src/binding.cjs', async () => {
  const { loadBinding } = await import('./src/load-binding');
  return loadBinding();
});

// These are the arguments the native side supplies. The hooks below only use
// the plain data and inner context, so no active native bundler is needed.
const ctx = { inner: () => ({}) };
const opts = { comments: {} };
const chunk = {};
const bundle = { chunks: [], assets: [] };
const code = 'export default 1';
const id = '/entry.js';
const changes = { changes: {}, deleted: new Set() };

const cases = [
  ['buildStart', [ctx, opts], 'ignored', undefined],
  ['buildEnd', [ctx, undefined], 'ignored', undefined],
  ['resolveId', [ctx, 'entry', null, {}], id, { id, normalizeExternalId: false }],
  ['resolveDynamicImport', [ctx, 'entry', null, {}], id, { id }],
  ['load', [ctx, id, undefined], code, { code }],
  ['transform', [ctx, code, id, { moduleType: 'js' }], code, { code }],
  ['moduleParsed', [ctx, { id }], 'ignored', undefined],
  ['renderStart', [ctx, opts], 'ignored', undefined],
  ['renderChunk', [ctx, code, chunk, opts, { chunks: {} }], code, { code }],
  ['augmentChunkHash', [ctx, chunk], 'hash', 'hash'],
  ['resolveFileUrl', [ctx, {}], 'url', 'url'],
  ['renderError', [ctx, []], 'ignored', undefined],
  ['generateBundle', [ctx, bundle, false, opts], 'ignored', changes],
  ['writeBundle', [ctx, bundle, opts], 'ignored', changes],
  ['closeBundle', [ctx, undefined], 'ignored', undefined],
  ['banner', [ctx, chunk], 'banner', 'banner'],
  ['footer', [ctx, chunk], 'footer', 'footer'],
  ['intro', [ctx, chunk], 'intro', 'intro'],
  ['outro', [ctx, chunk], 'outro', 'outro'],
  ['watchChange', [ctx, id, 'update'], 'ignored', undefined],
  ['hotUpdate', [ctx, { kind: 'update', file: id, modules: [] }], [id], [id]],
  ['closeWatcher', [ctx], 'ignored', undefined],
] as const;

describe.each([false, true])('native hook callbacks with timings=%s', (measureTimings) => {
  test.each(cases)(
    '%s preserves plain returns and awaits async returns',
    async (name, args, value, expected) => {
      const handler = vi.fn((): unknown => value);
      const onLog = vi.fn();
      const plugin = { name: 'test', [name]: handler };
      const binding = bindingifyPlugin(
        plugin,
        {},
        {},
        new PluginContextData(onLog, {}, [plugin], []),
        [],
        onLog,
        'info',
        false,
        measureTimings ? pluginTimingsRecorderFor({}) : undefined,
      );
      const invoke = () => (binding[name] as (...args: any[]) => unknown)(...args);

      // Equality without awaiting is intentional: a Promise here forces the
      // native bridge to allocate handlers even though the hook finished inline.
      expect(invoke()).toEqual(expected);

      handler.mockReturnValueOnce(Promise.resolve(value));
      await expect(invoke()).resolves.toEqual(expected);

      // Await used to assimilate thenables too, including ones without .catch().
      // oxlint-disable-next-line unicorn/no-thenable -- exercise Promise assimilation
      handler.mockReturnValueOnce({ then: (resolve: (value: unknown) => void) => resolve(value) });
      await expect(invoke()).resolves.toEqual(expected);

      // A single hook may alternate between synchronous and asynchronous calls.
      expect(invoke()).toEqual(expected);
      expect(handler).toHaveBeenCalledTimes(4);
    },
  );

  test.each(cases)('%s annotates throws and rejections', async (name, args) => {
    const handler = vi.fn();
    const onLog = vi.fn();
    const plugin = { name: 'test', [name]: handler };
    const binding = bindingifyPlugin(
      plugin,
      {},
      {},
      new PluginContextData(onLog, {}, [plugin], []),
      [],
      onLog,
      'info',
      false,
      measureTimings ? pluginTimingsRecorderFor({}) : undefined,
    );
    const invoke = () => (binding[name] as (...args: any[]) => unknown)(...args);
    const metadata = {
      code: 'PLUGIN_ERROR',
      plugin: 'test',
      hook: name,
      ...(name === 'transform' ? { id } : {}),
    };

    const thrown = new Error('synchronous failure');
    handler.mockImplementationOnce(() => {
      throw thrown;
    });
    expect(invoke).toThrow(thrown);
    expect(thrown).toMatchObject(metadata);

    const rejected = new Error('asynchronous failure');
    handler.mockRejectedValueOnce(rejected);
    await expect(invoke()).rejects.toBe(rejected);
    expect(rejected).toMatchObject(metadata);

    const invalidThen = new Error('then getter failure');
    handler.mockReturnValueOnce({
      // oxlint-disable-next-line unicorn/no-thenable -- a then getter may throw during assimilation
      get then() {
        throw invalidThen;
      },
    });
    expect(invoke).toThrow(invalidThen);
    expect(invalidThen).toMatchObject(metadata);
  });
});
