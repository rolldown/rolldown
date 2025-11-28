import type { InputOptions } from '..';
import type { BindingNormalizedOptions } from '../binding.cjs';
import { lazyProp } from '../decorators/lazy';
import type { LogHandler } from '../log/log-handler';
import { PlainObjectLike } from '../types/plain-object-like';

/** @category Plugin APIs */
export interface NormalizedInputOptions {
  input: string[] | Record<string, string>;
  cwd: string | undefined;
  platform: InputOptions['platform'];
  shimMissingExports: boolean;
  context: string;
}

export class NormalizedInputOptionsImpl extends PlainObjectLike
  implements NormalizedInputOptions
{
  inner: BindingNormalizedOptions;
  constructor(
    inner: BindingNormalizedOptions,
    public onLog: LogHandler,
  ) {
    super();
    this.inner = inner;
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
  get cwd(): string | undefined {
    return this.inner.cwd ?? undefined;
  }

  @lazyProp
  get platform(): 'browser' | 'node' | 'neutral' {
    return this.inner.platform;
  }

  @lazyProp
  get context(): string {
    return this.inner.context;
  }
}
