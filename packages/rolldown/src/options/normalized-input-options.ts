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
    // Read every option once, then drop the native object, which would keep the plugins alive.
    // See internal-docs/bundler-data-lifecycle/implementation.md ("Normalized options objects").
    for (const key of getLazyFields(this)) {
      Reflect.get(this, key);
    }
    // A getter that reads `inner` must be a `@lazyProp`: nothing may read `inner` after this line.
    this.inner = null as unknown as BindingNormalizedOptions;
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
