import { afterEach, describe, expect, it, vi } from 'vitest';
import { rolldown } from '../src/api/rolldown';
import type { InputOptions } from '../src/options/input-options';
import type { OutputOptions, PreRenderedAsset } from '../src/options/output-options';
import { createBundlerOptions } from '../src/utils/create-bundler-option';
import { summarizePluginTimings } from '../src/utils/plugin-timings';

vi.mock('../src/binding.cjs', async () => {
  const { loadBinding } = await import('./src/load-binding');
  return loadBinding();
});

function fakeClock() {
  let now = 1_000;
  const read = vi.spyOn(performance, 'now').mockImplementation(() => now);
  return {
    read,
    advance(ms: number) {
      now += ms;
    },
  };
}

afterEach(() => {
  vi.restoreAllMocks();
});

describe('assetFileNames timings', () => {
  it.each([undefined, true, false])(
    'measures only the callback through the binding with pluginTimings=%s',
    async (pluginTimings) => {
      const clock = fakeClock();
      const inputOptions: InputOptions = { checks: { pluginTimings } };
      const assetFileNames = vi.fn((asset: PreRenderedAsset) => {
        clock.advance(40);
        return `assets/${asset.names[0]}`;
      });
      const outputOptions: OutputOptions = { assetFileNames };
      const { bundlerOptions } = await createBundlerOptions(
        inputOptions,
        outputOptions,
        false,
        true,
      );
      const bindingAssetFileNames = bundlerOptions.outputOptions.assetFileNames;
      expect(typeof bindingAssetFileNames).toBe('function');
      if (typeof bindingAssetFileNames !== 'function') {
        throw new Error('Expected an assetFileNames callback');
      }

      clock.read.mockClear();
      expect(
        bindingAssetFileNames({
          name: 'example.txt',
          names: ['example.txt'],
          originalFileName: 'example.txt',
          originalFileNames: ['example.txt'],
          source: {
            get inner() {
              clock.advance(100);
              return 'asset contents';
            },
          },
        }),
      ).toBe('assets/example.txt');
      expect(assetFileNames).toHaveBeenCalledExactlyOnceWith({
        type: 'asset',
        name: 'example.txt',
        names: ['example.txt'],
        originalFileName: 'example.txt',
        originalFileNames: ['example.txt'],
        source: 'asset contents',
      });
      expect(outputOptions.assetFileNames).toBe(assetFileNames);

      if (pluginTimings === false) {
        expect(clock.read).not.toHaveBeenCalled();
        expect(bundlerOptions.inputOptions.pluginTimings).toBeUndefined();
        expect(summarizePluginTimings(inputOptions)).toEqual({ busyMs: 0, rows: [] });
      } else {
        expect(bundlerOptions.inputOptions.pluginTimings?.()).toEqual({
          busyMs: 40,
          rows: [
            {
              owner: 'output options',
              kind: 'outputOption',
              hook: 'assetFileNames',
              calls: 1,
              ms: 40,
              maxInFlight: 1,
              overlapMs: 0,
              rankable: true,
            },
          ],
        });
      }
    },
  );

  it.each(['function', 'string', 'disabled'] as const)(
    'preserves emitted asset names and records each callback once with %s options',
    async (mode) => {
      const clock = fakeClock();
      const pattern = 'assets/[name][extname]';
      const assetFileNames = vi.fn(() => {
        clock.advance(40);
        return pattern;
      });
      const inputOptions: InputOptions = {
        input: 'virtual:entry',
        checks: { pluginTimings: mode !== 'disabled' },
        plugins: [
          {
            name: 'emit-assets',
            resolveId: () => '\0entry',
            load: () => 'export default 1;',
            buildStart() {
              this.emitFile({ type: 'asset', name: 'first.txt', source: 'first' });
              this.emitFile({
                type: 'asset',
                name: 'second.bin',
                source: new Uint8Array([1, 2, 3]),
              });
              this.emitFile({ type: 'asset', fileName: 'explicit.txt', source: 'explicit' });
            },
          },
        ],
      };
      const bundle = await rolldown(inputOptions);
      try {
        const { output } = await bundle.generate({
          assetFileNames: mode === 'string' ? pattern : assetFileNames,
        });
        expect(output.filter((file) => file.type === 'asset').map((file) => file.fileName)).toEqual(
          ['assets/first.txt', 'assets/second.bin', 'explicit.txt'],
        );
        expect(assetFileNames).toHaveBeenCalledTimes(mode === 'string' ? 0 : 2);
        const measurement = summarizePluginTimings(inputOptions);
        expect(measurement.rows.filter((row) => row.hook === 'assetFileNames')).toEqual(
          mode === 'function'
            ? [
                {
                  owner: 'output options',
                  kind: 'outputOption',
                  hook: 'assetFileNames',
                  calls: 2,
                  ms: 80,
                  maxInFlight: 1,
                  overlapMs: 0,
                  rankable: true,
                },
              ]
            : [],
        );
        expect(measurement.busyMs).toBe(mode === 'function' ? 80 : 0);
      } finally {
        await bundle.close();
      }
    },
  );
});
