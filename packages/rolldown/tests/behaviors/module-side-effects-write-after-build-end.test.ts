import type { RolldownLog } from 'rolldown';
import { rolldown } from 'rolldown';
import { expect, test } from 'vitest';

const CODE = 'MODULE_SIDE_EFFECTS_WRITE_AFTER_BUILD_END';

const modules: Record<string, string> = {
  entry: 'import "a"; import "b"; console.log("entry");',
  a: 'console.log("A");',
  b: 'console.log("B");',
};

const virtualModules = {
  name: 'virtual-modules',
  resolveId: (id: string) => id,
  load: (id: string) => modules[id],
};

function collectWarnings() {
  const warnings: RolldownLog[] = [];
  return {
    warnings,
    onLog: (level: string, log: RolldownLog) => {
      if (level === 'warn' && log.code === CODE) warnings.push(log);
    },
  };
}

test('writes up to buildEnd do not warn', async () => {
  const { warnings, onLog } = collectWarnings();
  const build = await rolldown({
    input: 'entry',
    onLog,
    plugins: [
      virtualModules,
      {
        name: 'writer',
        transform(_code, id) {
          if (id === 'a') this.getModuleInfo(id)!.moduleSideEffects = true;
        },
        buildEnd() {
          this.getModuleInfo('b')!.moduleSideEffects = false;
        },
      },
    ],
  });
  try {
    await build.generate({});
  } finally {
    await build.close();
  }
  expect(warnings).toEqual([]);
});

test('only the first write after buildEnd warns', async () => {
  const { warnings, onLog } = collectWarnings();
  const build = await rolldown({
    input: 'entry',
    onLog,
    plugins: [
      virtualModules,
      {
        name: 'writer',
        renderStart() {
          this.getModuleInfo('a')!.moduleSideEffects = false;
          this.getModuleInfo('b')!.moduleSideEffects = false;
        },
      },
    ],
  });
  try {
    const { output } = await build.generate({});
    // The writes do not change tree shaking.
    expect(output[0].code).toContain('console.log("B")');
  } finally {
    await build.close();
  }
  expect(warnings).toEqual([expect.objectContaining({ code: CODE, id: 'a', plugin: 'writer' })]);
});

test('a write in the chunking context warns', async () => {
  const { warnings, onLog } = collectWarnings();
  const build = await rolldown({ input: 'entry', onLog, plugins: [virtualModules] });
  try {
    await build.generate({
      codeSplitting: {
        groups: [
          {
            name(id, ctx) {
              if (id === 'a') ctx.getModuleInfo(id)!.moduleSideEffects = false;
              return null;
            },
          },
        ],
      },
    });
  } finally {
    await build.close();
  }
  expect(warnings).toEqual([expect.objectContaining({ code: CODE, id: 'a' })]);
});
