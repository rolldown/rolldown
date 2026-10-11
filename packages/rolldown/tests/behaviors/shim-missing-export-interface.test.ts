import vm from 'node:vm';
import { rolldown, type InputOptions, type RolldownLog } from 'rolldown';
import { describe, expect, test } from 'vitest';
import { getOutputChunk } from '../src/utils';

async function build(modules: Record<string, string>, options: InputOptions = {}) {
  const warnings: RolldownLog[] = [];
  const bundle = await rolldown({
    input: 'main',
    shimMissingExports: true,
    onwarn: (warning) => warnings.push(warning),
    plugins: [{ name: 'virtual', resolveId: (id) => id, load: (id) => modules[id] }],
    ...options,
  });
  return { bundle, warnings };
}

const cases = [
  ['named import', 'import {x} from "dep"; export const value = x;'],
  ['unused named import', 'import {x} from "dep";'],
  ['named re-export', 'export {x as value} from "dep";'],
] as const;

describe.each(['es', 'cjs'] as const)('format: %s', (format) => {
  test.each(cases)('exposes shims through namespaces after a %s', async (_name, request) => {
    const { bundle, warnings } = await build({
      main: `${request}
      import * as ns from 'dep'; import * as barrel from 'barrel';
      export const keys = [Object.keys(ns).sort(), Object.keys(barrel).sort()];`,
      dep: 'export const value = 1;',
      barrel: 'export * from "dep";',
    });
    try {
      const [chunk] = getOutputChunk(await bundle.generate({ format }));
      const namespace =
        format === 'es'
          ? await import(`data:text/javascript,${encodeURIComponent(chunk.code)}`)
          : (() => {
              const context = { exports: {} };
              vm.runInNewContext(chunk.code, context);
              return context.exports;
            })();
      expect(namespace.keys).toEqual([
        ['value', 'x'],
        ['value', 'x'],
      ]);
      expect(namespace.value).toBeUndefined();
      expect(warnings).toEqual([
        expect.objectContaining({
          code: 'SHIMMED_EXPORT',
          binding: 'x',
          exporter: 'dep',
          message: expect.stringContaining('Missing export "x" has been shimmed in module "dep".'),
        }),
      ]);
    } finally {
      await bundle.close();
    }
  });
});

test('reports one warning per shim, including shims removed by tree-shaking', async () => {
  const { bundle, warnings } = await build({
    main: 'import {x, unused} from "dep"; import {y} from "other"; export {x,y};',
    other: 'import {x} from "dep"; export const y = x;',
    dep: 'export const value = 1;',
  });
  try {
    const [chunk] = getOutputChunk(await bundle.generate({ format: 'es' }));
    expect(chunk.code).not.toContain('unused');
    expect(
      warnings
        .map(({ code, exporter, binding }) => ({ code, exporter, binding }))
        .sort((a, b) => a.binding!.localeCompare(b.binding!)),
    ).toEqual([
      { code: 'SHIMMED_EXPORT', exporter: 'dep', binding: 'unused' },
      { code: 'SHIMMED_EXPORT', exporter: 'dep', binding: 'x' },
    ]);
  } finally {
    await bundle.close();
  }
});

test.each([false, true])(
  'keeps real star exports when static member reads are %s',
  async (read) => {
    const { bundle, warnings } = await build({
      main: `import {x} from 'a'; import * as ns from 'barrel'; export {x, ns};
      ${read ? 'export const read = ns.x;' : ''}`,
      a: 'export const z = 1;',
      b: 'export const x = 2;',
      barrel: 'export * from "a"; export * from "b";',
    });
    try {
      const [chunk] = getOutputChunk(await bundle.generate({ format: 'es' }));
      const namespace = await import(`data:text/javascript,${encodeURIComponent(chunk.code)}`);
      expect(namespace.ns.x).toBe(2);
      expect(namespace.x).toBeUndefined();
      expect(warnings).toEqual([
        expect.objectContaining({ code: 'SHIMMED_EXPORT', exporter: 'a', binding: 'x' }),
      ]);
    } finally {
      await bundle.close();
    }
  },
);

test('keeps genuine star conflicts ambiguous when another module shims the same name', async () => {
  const { bundle, warnings } = await build({
    main: 'import {x} from "missing"; import * as ns from "barrel"; export {x}; export const keys = Object.keys(ns);',
    missing: 'export {};',
    a: 'export const x = 1;',
    b: 'export const x = 2;',
    barrel: 'export * from "a"; export * from "b"; export * from "missing";',
  });
  try {
    const [chunk] = getOutputChunk(await bundle.generate({ format: 'es' }));
    const namespace = await import(`data:text/javascript,${encodeURIComponent(chunk.code)}`);
    expect(namespace.keys).toEqual([]);
    expect(warnings.map((warning) => warning.code).sort((a, b) => a!.localeCompare(b!))).toEqual([
      'NAMESPACE_CONFLICT',
      'SHIMMED_EXPORT',
    ]);
  } finally {
    await bundle.close();
  }
});

test('can disable shim warnings while preserving the shimmed interface', async () => {
  const { bundle, warnings } = await build(
    {
      main: 'import {x as value} from "dep"; import * as ns from "dep"; export {value}; export const keys = Object.keys(ns);',
      dep: 'export {};',
    },
    { checks: { shimmedExport: false } },
  );
  try {
    const [chunk] = getOutputChunk(await bundle.generate({ format: 'es' }));
    const namespace = await import(`data:text/javascript,${encodeURIComponent(chunk.code)}`);
    expect(namespace.keys).toEqual(['x']);
    expect(warnings).toEqual([]);
  } finally {
    await bundle.close();
  }
});

test('propagates shims through star cycles while preserving explicit exports and excluding default', async () => {
  const { bundle, warnings } = await build({
    main: 'import missing, {x, y} from "dep"; import * as ns from "barrel"; import * as cycle from "cycle"; export {missing,x,y,ns,cycle};',
    dep: 'export {};',
    barrel: 'export * from "cycle"; export * from "dep"; export const x = 42;',
    cycle: 'export * from "barrel";',
  });
  try {
    const [chunk] = getOutputChunk(await bundle.generate({ format: 'es' }));
    const namespace = await import(`data:text/javascript,${encodeURIComponent(chunk.code)}`);
    expect({ ...namespace.ns }).toEqual({ x: 42, y: undefined });
    expect({ ...namespace.cycle }).toEqual({ x: 42, y: undefined });
    expect(namespace.x).toBeUndefined();
    expect(warnings.map((warning) => warning.binding).sort((a, b) => a!.localeCompare(b!))).toEqual(
      ['default', 'x', 'y'],
    );
  } finally {
    await bundle.close();
  }
});

test('propagates silent implicit empty-module shims', async () => {
  const { bundle, warnings } = await build(
    {},
    {
      shimMissingExports: false,
      plugins: [
        {
          name: 'empty',
          resolveId: (id) => id,
          load: (id) =>
            id === 'dep'
              ? { code: '', moduleType: 'empty' }
              : id === 'barrel'
                ? 'export * from "dep";'
                : 'import {missing} from "dep"; import * as ns from "dep"; import * as barrel from "barrel"; export {missing}; export const keys = [Object.keys(ns), Object.keys(barrel)];',
        },
      ],
    },
  );
  try {
    const [chunk] = getOutputChunk(await bundle.generate({ format: 'es' }));
    const namespace = await import(`data:text/javascript,${encodeURIComponent(chunk.code)}`);
    expect(namespace.keys).toEqual([['missing'], ['missing']]);
    expect(warnings).toEqual([]);
  } finally {
    await bundle.close();
  }
});
