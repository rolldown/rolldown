import type { InputOptions, RolldownPlugin } from '..';
import type { BindingNormalizedOptions } from '../binding.cjs';
import { lazyProp } from '../decorators/lazy';
import type { LogHandler } from '../log/log-handler';
import { getLazyFields, PlainObjectLike } from '../types/plain-object-like';
import type { BindingCell } from '../utils/threadless-free';

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
  // Empty after `releaseBinding()`.
  private readonly bindingCell: BindingCell<BindingNormalizedOptions>;

  constructor(
    inner: BindingNormalizedOptions,
    public onLog: LogHandler,
    private inputPlugins: RolldownPlugin[],
  ) {
    super();
    this.bindingCell = { value: inner };
  }

  // Not a `#private` method: getters run with a Proxy or derived object as
  // `this`, which fails a `#private` brand check. `materializeBoxBackedFields()`
  // caches every field before the release, so no getter reaches this then.
  private binding(): BindingNormalizedOptions {
    const inner = this.bindingCell.value;
    if (inner === undefined) {
      throw new Error('These normalized input options have already released their binding object');
    }
    return inner;
  }

  /**
   * Evaluate and cache every lazy field so the native box can be released
   * while the wrapper keeps serving reads. Safe from the release path (which
   * must not run user code) because no lazy field here reads a user object.
   */
  materializeBoxBackedFields(): void {
    for (const field of getLazyFields(this)) {
      // property access is enough to evaluate and cache the lazy field
      void (this as Record<string, any>)[field];
    }
  }

  /**
   * Drop the binding wrapper once its box is released: the wrapper's methods
   * keep the binding instance that made it alive, so options a plugin keeps
   * would keep that instance's memory too.
   */
  releaseBinding(): void {
    this.bindingCell.value = undefined;
  }

  @lazyProp
  get shimMissingExports(): boolean {
    return this.binding().shimMissingExports;
  }

  @lazyProp
  get input(): string[] | Record<string, string> {
    return this.binding().input;
  }

  @lazyProp
  get cwd(): string {
    return this.binding().cwd;
  }

  @lazyProp
  get platform(): 'browser' | 'node' | 'neutral' {
    return this.binding().platform;
  }

  @lazyProp
  get context(): string {
    return this.binding().context;
  }

  get plugins(): RolldownPlugin[] {
    return this.inputPlugins;
  }
}
