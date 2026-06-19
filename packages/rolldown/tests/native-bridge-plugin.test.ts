import { rolldown } from 'rolldown';
import type { Plugin } from 'rolldown';
import { transformSync } from 'rolldown/utils';
import { describe, expect, it } from 'vitest';
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);
const binding = require('../src/binding.cjs') as {
  BenchOxcTransformer: new () => {
    transformNative(sourceHandle: bigint, id: string): bigint;
    transformNativeAsync(sourceHandle: bigint, id: string): Promise<bigint>;
  };
};

const SAMPLE_TSX = `
import * as React from 'react';
export function Counter() {
  const [n, setN] = React.useState(0);
  return <button onClick={() => setN(n + 1)}>{n}</button>;
}
`;

async function runWithBridge(bridgeKind: 'sync' | 'async') {
  const transformer = new binding.BenchOxcTransformer();
  let captured: string | undefined;

  const virtualEntry: Plugin = {
    name: 'virtual',
    resolveId(id) {
      if (id === 'entry.tsx') return id;
      if (id === 'react') return { id, external: true };
      return null;
    },
    load(id) {
      if (id === 'entry.tsx') return { code: SAMPLE_TSX, moduleType: 'tsx' };
      return null;
    },
  };

  const bridgePlugin = bridgeKind === 'sync'
    ? ({
      name: 'bridge-sync',
      transformNativeBridge(handle: bigint, id: string) {
        return transformer.transformNative(handle, id);
      },
    } as unknown as Plugin)
    : ({
      name: 'bridge-async',
      transformNativeBridgeAsync(handle: bigint, id: string) {
        return transformer.transformNativeAsync(handle, id);
      },
    } as unknown as Plugin);

  const capture: Plugin = {
    name: 'capture',
    transform(code) {
      captured = code;
      return null;
    },
  };

  const bundle = await rolldown({
    input: 'entry.tsx',
    plugins: [virtualEntry, bridgePlugin, capture],
  });
  await bundle.generate({ format: 'esm' });
  await bundle.close();

  return captured;
}

describe('native-bridge plugin paths', () => {
  it('sync bridge matches rolldown/utils transformSync', async () => {
    const expected = transformSync('Counter.tsx', SAMPLE_TSX, {
      reactCompiler: true,
    }).code;
    const actual = await runWithBridge('sync');
    expect(actual).toBeDefined();
    expect(actual).toBe(expected);
  });

  it('async bridge matches rolldown/utils transformSync', async () => {
    const expected = transformSync('Counter.tsx', SAMPLE_TSX, {
      reactCompiler: true,
    }).code;
    const actual = await runWithBridge('async');
    expect(actual).toBeDefined();
    expect(actual).toBe(expected);
  });
});
