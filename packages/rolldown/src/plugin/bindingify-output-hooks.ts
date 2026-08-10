import type {
  BindingHookFilter,
  BindingOutputs,
  BindingPluginContext,
  BindingPluginOptions,
} from '../binding.cjs';
import { RolldownMagicString } from '../binding-magic-string';
import { bindingifySourcemap } from '../types/sourcemap';
import { aggregateBindingErrorsIntoJsError, unwrapBindingResult } from '../utils/error';
import {
  dropBindingOutputs,
  releaseOrDefer,
  shouldEagerlyFreeOutputs,
} from '../utils/threadless-free';
import { snapshotRenderedChunk, transformRenderedChunk } from '../utils/transform-rendered-chunk';
import {
  type ChangedOutputs,
  collectChangedBundle,
  transformToOutputBundle,
} from '../utils/transform-to-rollup-output';
import { bindingifyRenderChunkFilter } from './bindingify-hook-filter';
import type { BindingifyPluginArgs } from './bindingify-plugin';
import { bindingifyHook, type PluginHookWithBindingExt } from './bindingify-plugin-hook-meta';
import { createPluginContext } from './plugin-context';

// Every hook invocation marshals fresh boxes (plugin context, rendered chunk,
// module info, normalized options). On the threadless-WASI flavor, where GC
// finalizers cannot be relied on, the wrappers release each box once its hook
// settles. Arguments a callback may legally retain are handed over as
// plain-data snapshots first, so a retained argument keeps working as data; a
// retained plugin context used after its hook settles throws a clear
// post-release error on this flavor only.
export function bindingifyRenderStart(
  args: BindingifyPluginArgs,
): PluginHookWithBindingExt<BindingPluginOptions['renderStart']> {
  return bindingifyHook(args.plugin.renderStart, ({ handler }) => ({
    plugin: async (ctx, opts) => {
      try {
        await handler.call(
          createPluginContext(args, ctx),
          args.pluginContextData.getOutputOptions(opts),
          args.pluginContextData.getInputOptions(opts),
        );
      } finally {
        releaseOrDefer(ctx);
      }
    },
  }));
}
export function bindingifyRenderChunk(
  args: BindingifyPluginArgs,
): PluginHookWithBindingExt<BindingPluginOptions['renderChunk'], BindingHookFilter | undefined> {
  return bindingifyHook(args.plugin.renderChunk, ({ handler, options }) => ({
    plugin: async (ctx, code, chunk, opts, meta) => {
      try {
        // cache the chunks binding to deduplicated avoid clone chunks
        if (args.pluginContextData.getRenderChunkMeta() == null) {
          args.pluginContextData.setRenderChunkMeta({
            chunks: Object.fromEntries(
              Object.entries(meta.chunks).map(([key, value]) => [
                key,
                // On the threadless flavor the cached meta must be plain data:
                // it outlives this invocation, and the per-chunk boxes minted
                // by `meta.chunks` would otherwise wait for finalizers.
                shouldEagerlyFreeOutputs()
                  ? snapshotRenderedChunk(value)
                  : transformRenderedChunk(value),
              ]),
            ),
          });
        }
        const renderChunkMeta = args.pluginContextData.getRenderChunkMeta()!;

        // Add lazy-loaded magicString if nativeMagicString is enabled
        let magicStringInstance: RolldownMagicString;
        if (args.options.experimental?.nativeMagicString) {
          Object.defineProperty(renderChunkMeta, 'magicString', {
            get() {
              if (magicStringInstance) {
                return magicStringInstance;
              }
              magicStringInstance = new RolldownMagicString(code);
              return magicStringInstance;
            },
            configurable: true,
          });
        }

        const ret = await handler.call(
          createPluginContext(args, ctx),
          code,
          shouldEagerlyFreeOutputs() ? snapshotRenderedChunk(chunk) : transformRenderedChunk(chunk),
          args.pluginContextData.getOutputOptions(opts),
          renderChunkMeta,
        );

        if (ret == null) {
          return;
        }

        // Handle MagicString return value directly
        if (ret instanceof RolldownMagicString) {
          const normalizedCode = ret.toString();
          const generatedMap = ret.generateMap();
          return {
            code: normalizedCode,
            map: bindingifySourcemap({
              file: generatedMap.file,
              mappings: generatedMap.mappings,
              names: generatedMap.names,
              sources: generatedMap.sources,
              sourcesContent: generatedMap.sourcesContent.map((s) => s ?? null),
            }),
          };
        }

        if (typeof ret === 'string') {
          return { code: ret };
        }

        // Handle object return with code as MagicString
        if (ret.code instanceof RolldownMagicString) {
          const magicString = ret.code as RolldownMagicString;
          const normalizedCode = magicString.toString();
          // If map is explicitly null, don't generate sourcemap (opt-out)
          // If map is undefined, auto-generate from MagicString
          if (ret.map === null) {
            return { code: normalizedCode, map: null };
          }
          if (ret.map === undefined) {
            const generatedMap = magicString.generateMap();
            return {
              code: normalizedCode,
              map: bindingifySourcemap({
                file: generatedMap.file,
                mappings: generatedMap.mappings,
                names: generatedMap.names,
                sources: generatedMap.sources,
                sourcesContent: generatedMap.sourcesContent.map((s) => s ?? null),
              }),
            };
          }
          return {
            code: normalizedCode,
            map: bindingifySourcemap(ret.map),
          };
        }

        if (ret.map === null) {
          return { code: ret.code, map: null };
        }

        return {
          code: ret.code,
          map: bindingifySourcemap(ret.map),
        };
      } finally {
        if (shouldEagerlyFreeOutputs()) {
          // The chunk box was released by `snapshotRenderedChunk`; the meta
          // box is read on the first invocation only (the snapshot above is
          // cached), so every invocation's copy can go, as can the context.
          meta.dropInner();
          releaseOrDefer(ctx);
        }
      }
    },
    filter: bindingifyRenderChunkFilter(options.filter),
  }));
}

export function bindingifyAugmentChunkHash(
  args: BindingifyPluginArgs,
): PluginHookWithBindingExt<BindingPluginOptions['augmentChunkHash']> {
  return bindingifyHook(args.plugin.augmentChunkHash, ({ handler }) => ({
    plugin: async (ctx, chunk) => {
      try {
        // The hook is typed sync, but a handler may still return a thenable at runtime and
        // release must follow its settlement.
        // oxlint-disable-next-line typescript/await-thenable
        return await handler.call(
          createPluginContext(args, ctx),
          shouldEagerlyFreeOutputs() ? snapshotRenderedChunk(chunk) : transformRenderedChunk(chunk),
        );
      } finally {
        releaseOrDefer(ctx);
      }
    },
  }));
}

export function bindingifyResolveFileUrl(
  args: BindingifyPluginArgs,
): PluginHookWithBindingExt<BindingPluginOptions['resolveFileUrl']> {
  return bindingifyHook(args.plugin.resolveFileUrl, ({ handler }) => ({
    plugin: async (ctx, resolveFileUrlArgs) => {
      try {
        // The hook is typed sync, but a handler may still return a thenable at runtime and
        // release must follow its settlement.
        // oxlint-disable-next-line typescript/await-thenable
        return await handler.call(createPluginContext(args, ctx), resolveFileUrlArgs);
      } finally {
        releaseOrDefer(ctx);
      }
    },
  }));
}

export function bindingifyRenderError(
  args: BindingifyPluginArgs,
): PluginHookWithBindingExt<BindingPluginOptions['renderError']> {
  return bindingifyHook(args.plugin.renderError, ({ handler }) => ({
    plugin: async (ctx, err) => {
      try {
        await handler.call(createPluginContext(args, ctx), aggregateBindingErrorsIntoJsError(err));
      } finally {
        releaseOrDefer(ctx);
      }
    },
  }));
}

function createOutputBundle(
  args: BindingifyPluginArgs,
  ctx: BindingPluginContext,
  outputs: BindingOutputs,
) {
  const changed = {
    updated: new Set(),
    deleted: new Set(),
  } as ChangedOutputs;
  const context = createPluginContext(args, ctx);
  const output = transformToOutputBundle(context, outputs, changed);
  return { changed, context, output };
}

// Each generateBundle/writeBundle invocation gets its own marshaled bundle
// copy: fresh boxes sharing the build's native `Arc`s (`js_plugin.rs` marshals
// `args.bundle.clone()`). On the threadless-WASI flavor they must be released
// after `collectChangedBundle` finished its native reads and before Rust
// applies the changes, so Rust sees our references gone and can `Arc::get_mut`
// in place. Plugins that stash `bundle` past the hook then get throwing getters
// for not-yet-read fields, on this flavor only.
export function bindingifyGenerateBundle(
  args: BindingifyPluginArgs,
): PluginHookWithBindingExt<BindingPluginOptions['generateBundle']> {
  return bindingifyHook(args.plugin.generateBundle, ({ handler }) => ({
    plugin: async (ctx, bundle, isWrite, opts) => {
      const outputs = unwrapBindingResult(bundle);
      try {
        const { changed, context, output } = createOutputBundle(args, ctx, outputs);
        await handler.call(context, args.pluginContextData.getOutputOptions(opts), output, isWrite);
        return collectChangedBundle(changed, output);
      } finally {
        if (shouldEagerlyFreeOutputs()) {
          dropBindingOutputs(outputs);
          releaseOrDefer(ctx);
        }
      }
    },
  }));
}

export function bindingifyWriteBundle(
  args: BindingifyPluginArgs,
): PluginHookWithBindingExt<BindingPluginOptions['writeBundle']> {
  return bindingifyHook(args.plugin.writeBundle, ({ handler }) => ({
    plugin: async (ctx, bundle, opts) => {
      const outputs = unwrapBindingResult(bundle);
      try {
        const { changed, context, output } = createOutputBundle(args, ctx, outputs);
        await handler.call(context, args.pluginContextData.getOutputOptions(opts), output);
        return collectChangedBundle(changed, output);
      } finally {
        if (shouldEagerlyFreeOutputs()) {
          dropBindingOutputs(outputs);
          releaseOrDefer(ctx);
        }
      }
    },
  }));
}

export function bindingifyCloseBundle(
  args: BindingifyPluginArgs,
): PluginHookWithBindingExt<BindingPluginOptions['closeBundle']> {
  return bindingifyHook(args.plugin.closeBundle, ({ handler }) => ({
    plugin: async (ctx, err) => {
      try {
        const invokeHook = () =>
          handler.call(
            createPluginContext(args, ctx),
            err ? aggregateBindingErrorsIntoJsError(err) : undefined,
          );
        await (args.closeCallbackScope
          ? args.closeCallbackScope.runWithCloseIdentity(ctx.closeIdentity(), invokeHook)
          : invokeHook());
      } finally {
        releaseOrDefer(ctx);
      }
    },
  }));
}

export function bindingifyAddonHook<K extends 'banner' | 'footer' | 'intro' | 'outro'>(
  args: BindingifyPluginArgs,
  name: K,
): PluginHookWithBindingExt<BindingPluginOptions[K]> {
  return bindingifyHook(args.plugin[name], ({ handler }) => ({
    plugin: async (ctx, chunk) => {
      if (typeof handler === 'string') {
        if (shouldEagerlyFreeOutputs()) {
          // A string addon never reads its boxes; release them right away.
          chunk.dropInner();
          releaseOrDefer(ctx);
        }
        return handler;
      }

      try {
        return await handler.call(
          createPluginContext(args, ctx),
          shouldEagerlyFreeOutputs() ? snapshotRenderedChunk(chunk) : transformRenderedChunk(chunk),
        );
      } finally {
        releaseOrDefer(ctx);
      }
    },
  }));
}
