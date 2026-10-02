import { afterEach, describe, expect, it, vi } from 'vitest';
import { rolldown } from '../src/api/rolldown';
import type { InputOptions } from '../src/options/input-options';
import type { OutputOptions } from '../src/options/output-options';
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

describe('sanitizeFileName timings', () => {
  it.each([undefined, true, false])(
    'measures the callback through the binding with pluginTimings=%s',
    async (pluginTimings) => {
      const clock = fakeClock();
      const inputOptions: InputOptions = { checks: { pluginTimings } };
      const sanitizeFileName = vi.fn((name: string) => {
        clock.advance(40);
        return `sanitized-${name}`;
      });
      const outputOptions: OutputOptions = { sanitizeFileName };
      const { bundlerOptions } = await createBundlerOptions(
        inputOptions,
        outputOptions,
        false,
        undefined,
        undefined,
        undefined,
        true,
      );
      const bindingSanitizeFileName = bundlerOptions.outputOptions.sanitizeFileName;
      expect(typeof bindingSanitizeFileName).toBe('function');
      if (typeof bindingSanitizeFileName !== 'function') {
        throw new Error('Expected a sanitizeFileName callback');
      }

      clock.advance(100);
      clock.read.mockClear();
      expect(bindingSanitizeFileName('+example.txt')).toBe('sanitized-+example.txt');
      expect(sanitizeFileName).toHaveBeenCalledExactlyOnceWith('+example.txt');
      expect(outputOptions.sanitizeFileName).toBe(sanitizeFileName);

      if (pluginTimings === false) {
        expect(bindingSanitizeFileName).toBe(sanitizeFileName);
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
              hook: 'sanitizeFileName',
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

  it.each(['function', true, false, 'disabled'] as const)(
    'preserves asset and chunk names and records each callback once with %s options',
    async (mode) => {
      const clock = fakeClock();
      const sanitizeFileName = vi.fn((name: string) => {
        clock.advance(40);
        return `sanitized-${name}`;
      });
      const inputOptions: InputOptions = {
        input: { entry: 'virtual:entry' },
        checks: { pluginTimings: mode !== 'disabled' },
        plugins: [
          {
            name: 'emit-assets',
            resolveId: () => '\0entry',
            load: () => 'export default 1;',
            buildStart() {
              this.emitFile({ type: 'asset', name: '+emitted.txt', source: 'emitted' });
              this.emitFile({ type: 'asset', source: 'unnamed' });
              this.emitFile({ type: 'asset', fileName: '+explicit.txt', source: 'explicit' });
            },
          },
        ],
      };
      const bundle = await rolldown(inputOptions);
      try {
        const { output } = await bundle.generate({
          assetFileNames: 'assets/[name][extname]',
          sanitizeFileName: typeof mode === 'boolean' ? mode : sanitizeFileName,
        });
        expect(output.map((file) => file.fileName).sort()).toEqual(
          typeof mode === 'boolean'
            ? [
                '+explicit.txt',
                mode ? 'assets/_emitted.txt' : 'assets/+emitted.txt',
                'assets/asset',
                'entry.js',
              ]
            : [
                '+explicit.txt',
                'assets/sanitized-+emitted.txt',
                'assets/sanitized-asset',
                'sanitized-entry.js',
              ],
        );
        expect(sanitizeFileName.mock.calls).toEqual(
          typeof mode === 'boolean' ? [] : [['+emitted.txt'], ['asset'], ['entry']],
        );
        const measurement = summarizePluginTimings(inputOptions);
        expect(measurement.rows.filter((row) => row.hook === 'sanitizeFileName')).toEqual(
          mode === 'function'
            ? [
                {
                  owner: 'output options',
                  kind: 'outputOption',
                  hook: 'sanitizeFileName',
                  calls: 3,
                  ms: 120,
                  maxInFlight: 1,
                  overlapMs: 0,
                  rankable: true,
                },
              ]
            : [],
        );
        expect(measurement.busyMs).toBe(mode === 'function' ? 120 : 0);
      } finally {
        await bundle.close();
      }
    },
  );
});
