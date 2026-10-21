import type { BindingOutputAsset, ExternalMemoryStatus } from '../binding.cjs';
import { lazyProp } from '../decorators/lazy';
import type { AssetSource } from '../utils/asset-source';
import { transformAssetSource } from '../utils/asset-source';
import { getLazyFields, PlainObjectLike } from './plain-object-like';
import type { OutputAsset } from './rolldown-output';

export class OutputAssetImpl extends PlainObjectLike implements OutputAsset {
  readonly type = 'asset' as const;

  // Undefined after `releaseBindings()`.
  private bindingAsset: BindingOutputAsset | undefined;

  constructor(bindingAsset: BindingOutputAsset) {
    super();
    this.bindingAsset = bindingAsset;
  }

  // Every field is cached before the release, so no getter reaches this then.
  #binding(): BindingOutputAsset {
    if (this.bindingAsset === undefined) {
      throw new Error('This output asset has already released its binding object');
    }
    return this.bindingAsset;
  }

  @lazyProp
  get fileName(): string {
    return this.#binding().getFileName();
  }

  @lazyProp
  get originalFileName(): string | null {
    return this.#binding().getOriginalFileName() || null;
  }

  @lazyProp
  get originalFileNames(): string[] {
    return this.#binding().getOriginalFileNames();
  }

  @lazyProp
  get name(): string | undefined {
    return this.#binding().getName() ?? undefined;
  }

  @lazyProp
  get names(): string[] {
    return this.#binding().getNames();
  }

  @lazyProp
  get source(): AssetSource {
    return transformAssetSource(this.#binding().getSource());
  }

  __rolldown_external_memory_handle__(keepDataAlive?: boolean): ExternalMemoryStatus {
    if (keepDataAlive) {
      this.#evaluateAllLazyFields();
    }
    if (this.bindingAsset === undefined) {
      return { freed: false, reason: 'Memory has already been freed' };
    }
    return this.bindingAsset.dropInner();
  }

  /** @internal Cache every field, then drop the binding wrapper. */
  releaseBindings(): void {
    if (this.bindingAsset === undefined) return;
    this.#evaluateAllLazyFields();
    this.bindingAsset = undefined;
  }

  #evaluateAllLazyFields(): void {
    for (const field of getLazyFields(this)) {
      // Accessing the property triggers lazy evaluation via the @lazyProp decorator.
      const _value = (this as any)[field];
    }
  }
}
