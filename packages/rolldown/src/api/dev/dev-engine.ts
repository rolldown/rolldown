import {
  type BindingBundleState,
  BindingDevEngine,
  type BindingDevOptions,
  type BindingHmrUpdate,
  type BindingLazyChunkResponse,
  type BindingModuleInfo,
  type BindingPatchResponse,
  BindingRebuildStrategy,
  type BindingResult,
  shutdownAsyncRuntime,
  startAsyncRuntime,
} from '../../binding.cjs';
import type { InputOptions } from '../../options/input-options';
import type { OutputOptions } from '../../options/output-options';
import { PluginDriver } from '../../plugin/plugin-driver';
import { createBundlerOptions } from '../../utils/create-bundler-option';
import { normalizeBindingResult, unwrapBindingResult } from '../../utils/error';
import { normalizedStringOrRegex } from '../../utils/normalize-string-or-regex';
import {
  trackOutputExternalMemory,
  transformToRollupOutput,
} from '../../utils/transform-to-rollup-output';
import type { DevOptions } from './dev-options';

/**
 * The part of the binding engine the module graph reads from.
 *
 * Typed structurally instead of as `BindingDevEngine`: a public constructor
 * parameter type is emitted into the public dts, and naming the binding class
 * there would pull the whole binding type chain into the public surface.
 */
interface ModuleGraphSource {
  getModuleInfo(moduleId: string): BindingModuleInfo | null;
  getModuleIds(): Array<string>;
}

/** Read-only view over the engine's module graph, kept current across rebuilds. */
export class DevEngineModuleGraph {
  #inner: ModuleGraphSource;

  constructor(inner: ModuleGraphSource) {
    this.#inner = inner;
  }

  /**
   * Get additional information about the module in question.
   *
   * @returns Module information for that module. `null` if the module could not be found.
   */
  getModuleInfo(moduleId: string): BindingModuleInfo | null {
    return this.#inner.getModuleInfo(moduleId) ?? null;
  }

  /**
   * Get all module ids in the current module graph.
   *
   * @returns An array of module ids.
   */
  getModuleIds(): string[] {
    return this.#inner.getModuleIds();
  }
}

export class DevEngine {
  #inner: BindingDevEngine;
  #cachedBuildFinishPromise: Promise<void> | null = null;
  #asyncRuntimeReleased = false;

  readonly moduleGraph: DevEngineModuleGraph;

  static async create(
    inputOptions: InputOptions,
    outputOptions: OutputOptions = {},
    devOptions: DevOptions = {},
  ): Promise<DevEngine> {
    inputOptions = await PluginDriver.callOptionsHook(inputOptions);
    const bundlerOptions = await createBundlerOptions(inputOptions, outputOptions, false);

    const userOnHmrUpdates = devOptions.onHmrUpdates;
    const bindingOnHmrUpdates: BindingDevOptions['onHmrUpdates'] = userOnHmrUpdates
      ? function (rawResult: BindingResult<[BindingHmrUpdate, string[]]>) {
          const result = normalizeBindingResult(rawResult);
          if (result instanceof Error) {
            userOnHmrUpdates(result);
            return;
          }
          const [update, changedFiles] = result;
          userOnHmrUpdates({
            update,
            changedFiles,
          });
        }
      : undefined;

    const userOnOutput = devOptions.onOutput;
    const bindingOnOutput: BindingDevOptions['onOutput'] = userOnOutput
      ? function (rawResult, hmrGeneration) {
          const result = normalizeBindingResult(rawResult);
          if (result instanceof Error) {
            userOnOutput(result);
            return;
          }
          trackOutputExternalMemory(result);
          userOnOutput(
            transformToRollupOutput(result),
            hmrGeneration == null ? undefined : { hmrGeneration },
          );
        }
      : undefined;

    const userOnAdditionalAssets = devOptions.onAdditionalAssets;
    const bindingOnAdditionalAssets: BindingDevOptions['onAdditionalAssets'] =
      userOnAdditionalAssets
        ? function (output) {
            trackOutputExternalMemory(output);
            userOnAdditionalAssets(transformToRollupOutput(output));
          }
        : undefined;

    const bindingDevOptions: BindingDevOptions = {
      onHmrUpdates: bindingOnHmrUpdates,
      onOutput: bindingOnOutput,
      onAdditionalAssets: bindingOnAdditionalAssets,
      rebuildStrategy: devOptions.rebuildStrategy
        ? devOptions.rebuildStrategy === 'always'
          ? BindingRebuildStrategy.Always
          : BindingRebuildStrategy.Never
        : undefined,
      watch: devOptions.watch && {
        enabled: devOptions.watch.enabled,
        skipWrite: devOptions.watch.skipWrite,
        watcher: devOptions.watch.watcher,
        include: normalizedStringOrRegex(devOptions.watch.include),
        exclude: normalizedStringOrRegex(devOptions.watch.exclude),
      },
      hotUpdate: devOptions.hotUpdate,
    };

    const inner = new BindingDevEngine(bundlerOptions, bindingDevOptions);

    startAsyncRuntime();

    return new DevEngine(inner);
  }

  private constructor(inner: BindingDevEngine) {
    this.#inner = inner;
    this.moduleGraph = new DevEngineModuleGraph(inner);
  }

  async run(): Promise<void> {
    await this.#inner.run();
  }

  async ensureCurrentBuildFinish(): Promise<void> {
    if (this.#cachedBuildFinishPromise) {
      return this.#cachedBuildFinishPromise;
    }
    const promise = this.#inner.ensureCurrentBuildFinish().then(() => {
      this.#cachedBuildFinishPromise = null;
    });
    this.#cachedBuildFinishPromise = promise;
    return promise;
  }

  async getBundleState(): Promise<BindingBundleState> {
    return this.#inner.getBundleState();
  }

  async ensureLatestBuildOutput(): Promise<void> {
    unwrapBindingResult(await this.#inner.ensureLatestBuildOutput());
  }

  triggerFullBuild(): void {
    this.#inner.triggerFullBuild();
  }

  /**
   * Render the patch a client asks for.
   *
   * @param query - The query string of the client's patch request,
   * `s=<epoch>&g=<gen>&u=<set>&e=<set>&f=<set>`. Each set is `b` and a
   * base64url bitset. Other keys are ignored.
   * @returns The patch, or `{ type: 'Reload' }` when the client must reload (it
   * belongs to another engine, its generation is too old to catch up, or the
   * query is bad). With `sourcemap: true` or `'hidden'`, `sourcemap` is the map
   * as JSON and `code` has no `sourceMappingURL` comment; the caller serves the
   * map. With `'inline'`, `code` ends with an inline map.
   */
  async renderPatch(query: string): Promise<BindingPatchResponse> {
    return unwrapBindingResult(await this.#inner.renderPatch(query));
  }

  async close(): Promise<void> {
    // Claim the release before the first await so a second `close` cannot release twice.
    const shouldRelease = !this.#asyncRuntimeReleased;
    this.#asyncRuntimeReleased = true;
    try {
      await this.#inner.close();
    } finally {
      if (shouldRelease) {
        shutdownAsyncRuntime();
      }
    }
  }

  /**
   * Compile a lazy entry module and return HMR-style patch code.
   *
   * This is called when a dynamically imported module is first requested at runtime.
   * The module was previously stubbed with a proxy, and now we need to compile the
   * actual module and its dependencies.
   *
   * @param moduleId - The absolute file path of the module to compile
   * @param query - The query string of the client's lazy request, which
   * describes what the client holds in the format of {@link renderPatch}
   * @returns The compiled chunk, or `{ type: 'Reload' }` when the client must
   * reload. `code` and `sourcemap` follow the rules of {@link renderPatch}.
   * @throws The plugin's own error, since a lazy compile runs the full pipeline and is
   * where a user plugin most often throws
   */
  async compileEntry(moduleId: string, query: string): Promise<BindingLazyChunkResponse> {
    return unwrapBindingResult(await this.#inner.compileEntry(moduleId, query));
  }
}
