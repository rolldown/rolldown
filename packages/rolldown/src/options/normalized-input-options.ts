import type { InputOptions } from '..';
import { BindingNormalizedOptions } from '../binding';
import type { LogHandler } from '../types/misc';

export interface NormalizedInputOptions {
  input: string[] | Record<string, string>;
  cwd: string | undefined;
  platform: InputOptions['platform'];
  shimMissingExports: boolean;
}

// TODO: I guess we make these getters enumerable so it act more like a plain object
export class NormalizedInputOptionsImpl implements NormalizedInputOptions {
  inner: BindingNormalizedOptions;
  constructor(
    inner: BindingNormalizedOptions,
    public onLog: LogHandler,
  ) {
    this.inner = inner;
  }

  get shimMissingExports(): boolean {
    return this.inner.shimMissingExports;
  }

  get input(): string[] | Record<string, string> {
    return this.inner.input;
  }

  get cwd(): string | undefined {
    return this.inner.cwd ?? undefined;
  }

  get platform(): 'browser' | 'node' | 'neutral' {
    return this.inner.platform;
  }
}
