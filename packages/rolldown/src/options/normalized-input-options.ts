import type { InputOptions, RolldownPlugin } from '..';
import type { BindingNormalizedOptions } from '../binding.cjs';
import { lazyProp } from '../decorators/lazy';
import type { LogHandler } from '../log/log-handler';
import { getLazyFields, PlainObjectLike } from '../types/plain-object-like';

/** @category Plugin APIs */
export interface NormalizedInputOptions {
  /** @see {@linkcode InputOptions.input | input} */
  input: string[] | Record<string, string>;
  /** @see {@linkcode InputOptions.cwd | cwd} */
  cwd: string;
  /** @see {@linkcode InputOptions.platform | platform} */
  platform: InputOptions['platform'];
  /** @see {@linkcode InputOptions.shimMissingExports | shimMissingExports} */
  shimMissingExports: boolean;
  /** @see {@linkcode InputOptions.context | context} */
  context: string;
  /** @see {@linkcode InputOptions.plugins | plugins} */
  plugins: RolldownPlugin[];
}

export class NormalizedInputOptionsImpl extends PlainObjectLike implements NormalizedInputOptions {
  inner: BindingNormalizedOptions;

  constructor(
    inner: BindingNormalizedOptions,
    public onLog: LogHandler,
    private inputPlugins: RolldownPlugin[],
  ) {
    super();
    this.inner = inner;
  }

  /**
   * Evaluates and caches every lazy field so the native box can be released
   * while the wrapper keeps serving reads. Every lazy field on this class
   * reads only the native box — never a user-provided object — so this is
   * safe to call from the release path, which must not execute user code.
   */
  materializeBoxBackedFields(): void {
    for (const field of getLazyFields(this)) {
      // property access is enough to evaluate and cache the lazy field
      void (this as Record<string, any>)[field];
    }
  }

  @lazyProp
  get shimMissingExports(): boolean {
    return this.inner.shimMissingExports;
  }

  @lazyProp
  get input(): string[] | Record<string, string> {
    return this.inner.input;
  }

  @lazyProp
  get cwd(): string {
    return this.inner.cwd;
  }

  @lazyProp
  get platform(): 'browser' | 'node' | 'neutral' {
    return this.inner.platform;
  }

  @lazyProp
  get context(): string {
    return this.inner.context;
  }

  get plugins(): RolldownPlugin[] {
    return this.inputPlugins;
  }
}
