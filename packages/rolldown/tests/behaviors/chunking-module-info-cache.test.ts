import { isThreadlessWasi } from '@tests/runtime-flavor';
import type { ChunkingContext, ModuleInfo, OutputOptions, PluginContext } from 'rolldown';
import { rolldown } from 'rolldown';
import { expect, test } from 'vitest';

const modules: Record<string, string> = {
  entry: 'import "dep"; import "external"; console.log("entry");',
  dep: 'console.log("dep"); export const value = 1;',
};

// Some checks below read a context AFTER the invocation that owns it: the
// plugin context `buildEnd` was handed (kept alive through the captured
// `getModuleInfo`), and a group-`name` batch's chunking context (read again
// from `renderChunk` and `renderError`, where the pass-scoped module-info cache
// has already been invalidated so the read reaches the native box).
//
// The threadless-WASI flavor never runs the GC finalizers those boxes would
// otherwise wait for, so the wrappers release them the moment their invocation
// settles -- `bindingify-build-hooks.ts` `buildEnd`'s `finally` and
// `bindingify-output-options.ts` `batchName`'s `finally`, both through
// `src/utils/threadless-free.ts`. The late read then throws from the binding:
//   plugin context    "...this plugin context's native data was eagerly released
//                      after its hook invocation settled."
//                     crates/rolldown_binding/src/options/plugin/binding_plugin_context.rs
//   chunking context  "...this chunking context's native data was eagerly released
//                      after its group-name invocation settled."
//                     crates/rolldown_binding/src/options/binding_output_options/binding_manual_code_splitting_options.rs
//
// Asserting that throw instead of skipping the flavor keeps the module-info
// cache contract itself covered on the WASI lane -- the same call
// `fixtures/topics/free-external-memory/_config.ts` makes for `freeExternalMemory()`.
const RELEASED_PLUGIN_CONTEXT = /this plugin context's native data was eagerly released/;
const RELEASED_CHUNKING_CONTEXT = /this chunking context's native data was eagerly released/;

test('shares module info across groups and releases it before rendering', async () => {
  let getPluginModuleInfo: PluginContext['getModuleInfo'];
  let chunkingContext: ChunkingContext;
  let firstInfo: ModuleInfo | undefined;
  let groupCalls = 0;
  let renderCalls = 0;
  const build = await rolldown({
    input: 'entry',
    external: ['external'],
    plugins: [
      {
        name: 'virtual-modules',
        resolveId: (id) => id,
        load: (id) => modules[id],
        buildEnd() {
          getPluginModuleInfo = this.getModuleInfo;
        },
        renderChunk() {
          renderCalls++;
          if (isThreadlessWasi) {
            expect(() => chunkingContext.getModuleInfo('dep')).toThrow(RELEASED_CHUNKING_CONTEXT);
            // This hook's own plugin context is live, so the write-through
            // between a freshly minted module info and the shared option store
            // still holds here.
            const live = this.getModuleInfo('dep')!;
            live.moduleSideEffects = false;
            expect(this.getModuleInfo('dep')!.moduleSideEffects).toBe(false);
            this.getModuleInfo('dep')!.moduleSideEffects = true;
            expect(live.moduleSideEffects).toBe(true);
          } else {
            const info = chunkingContext.getModuleInfo('dep')!;
            expect(info === firstInfo).toBe(false);
            expect(chunkingContext.getModuleInfo('dep') === info).toBe(false);
            info.moduleSideEffects = false;
            expect(this.getModuleInfo('dep')!.moduleSideEffects).toBe(false);
            this.getModuleInfo('dep')!.moduleSideEffects = true;
            expect(info.moduleSideEffects).toBe(true);
          }
        },
      },
    ],
  });
  try {
    await build.generate({
      codeSplitting: {
        groups: [0, 1].map(() => ({
          name(_id, context) {
            groupCalls++;
            chunkingContext = context;
            const info = context.getModuleInfo('dep')!;
            firstInfo ??= info;
            expect(info).toBe(firstInfo);
            expect(info.importers).toEqual(['entry']);
            expect(context.getModuleInfo('external')).toBe(context.getModuleInfo('external'));
            expect(context.getModuleInfo('missing')).toBeNull();
            if (isThreadlessWasi) {
              // `getPluginModuleInfo` is bound to the box `buildEnd` released.
              expect(() => getPluginModuleInfo('dep')).toThrow(RELEASED_PLUGIN_CONTEXT);
              info.moduleSideEffects = true;
              expect(info.moduleSideEffects).toBe(true);
              info.moduleSideEffects = null;
              expect(info.moduleSideEffects).toBeNull();
              info.meta.group = 'shared';
              expect(info.meta.group).toBe('shared');
            } else {
              getPluginModuleInfo('dep')!.moduleSideEffects = true;
              expect(info.moduleSideEffects).toBe(true);
              getPluginModuleInfo('dep')!.moduleSideEffects = false;
              expect(info.moduleSideEffects).toBe(false);
              info.moduleSideEffects = true;
              expect(getPluginModuleInfo('dep')!.moduleSideEffects).toBe(true);
              info.moduleSideEffects = null;
              expect(info.moduleSideEffects).toBeNull();
              expect(getPluginModuleInfo('dep')!.moduleSideEffects).toBeNull();
              expect(info.meta).toBe(getPluginModuleInfo('dep')!.meta);
              info.meta.group = 'shared';
              expect(getPluginModuleInfo('dep')!.meta.group).toBe('shared');
            }
            return null;
          },
        })),
      },
    });
    expect(groupCalls).toBeGreaterThanOrEqual(4);
    expect(renderCalls).toBeGreaterThan(0);
  } finally {
    await build.close();
  }
});

test.each(['name throws', 'name returns invalid type', 'later test throws'])(
  'releases cached module info when %s',
  async (failure) => {
    let context: ChunkingContext;
    let cached: ModuleInfo;
    let fail = true;
    let checkedError = false;
    const build = await rolldown({
      input: 'entry',
      external: ['external'],
      plugins: [
        {
          name: 'virtual-modules',
          resolveId: (id) => id,
          load: (id) => modules[id],
          renderError() {
            checkedError = true;
            if (isThreadlessWasi) {
              expect(() => context.getModuleInfo('dep')).toThrow(RELEASED_CHUNKING_CONTEXT);
              // This hook's own plugin context is live; the outer
              // `cached !== previousInfo` check below is what proves the
              // pass-scoped cache was dropped when the classifier failed.
              const info = this.getModuleInfo('dep')!;
              info.moduleSideEffects = null;
              expect(this.getModuleInfo('dep')!.moduleSideEffects).toBeNull();
              this.getModuleInfo('dep')!.moduleSideEffects = false;
              expect(info.moduleSideEffects).toBe(false);
            } else {
              const info = context.getModuleInfo('dep')!;
              expect(info === cached).toBe(false);
              expect(context.getModuleInfo('dep') === info).toBe(false);
              info.moduleSideEffects = null;
              expect(this.getModuleInfo('dep')!.moduleSideEffects).toBeNull();
              this.getModuleInfo('dep')!.moduleSideEffects = false;
              expect(info.moduleSideEffects).toBe(false);
            }
          },
        },
      ],
    });
    const output: OutputOptions = {
      codeSplitting: {
        groups: [
          {
            name(_id, ctx) {
              context = ctx;
              cached = ctx.getModuleInfo('dep')!;
              expect(ctx.getModuleInfo('dep')).toBe(cached);
              if (fail && failure === 'name throws') throw new Error('classifier failure');
              if (fail && failure === 'name returns invalid type') return 1 as unknown as string;
              return null;
            },
          },
          {
            name: 'second',
            test() {
              if (fail) throw new Error('classifier failure');
              return false;
            },
          },
        ],
      },
    };
    try {
      await expect(build.generate(output)).rejects.toThrow(
        failure === 'name returns invalid type' ? 'expected a string' : 'classifier failure',
      );
      expect(checkedError).toBe(true);
      const previousInfo = cached!;
      fail = false;
      await build.generate(output);
      expect(cached! === previousInfo).toBe(false);
    } finally {
      await build.close();
    }
  },
);

test('manualChunks reuses module info within each output', async () => {
  let previous: ModuleInfo | undefined;
  const build = await rolldown({
    input: 'entry',
    external: ['external'],
    plugins: [
      {
        name: 'virtual-modules',
        resolveId: (id) => id,
        load: (id) => modules[id],
      },
    ],
  });
  try {
    for (let pass = 0; pass < 2; pass++) {
      let current: ModuleInfo | undefined;
      await build.generate({
        manualChunks(_id, context) {
          const info = context.getModuleInfo('dep')!;
          current ??= info;
          expect(info).toBe(current);
          expect(info === previous).toBe(false);
          return null;
        },
      });
      expect(current).toBeDefined();
      previous = current;
    }
  } finally {
    await build.close();
  }
});
