import type { BindingOutputAsset } from '../binding.cjs';
import { lazyProp } from '../decorators/lazy';
import type { AssetSource } from '../utils/asset-source';
import { transformAssetSource } from '../utils/asset-source';
import { PlainObjectLike } from './plain-object-like';
import type { OutputAsset } from './rolldown-output';

export class OutputAssetImpl extends PlainObjectLike implements OutputAsset {
  readonly type = 'asset' as const;

  constructor(private bindingAsset: BindingOutputAsset) {
    super();
  }

  @lazyProp
  get fileName(): string {
    return this.bindingAsset.getFileName();
  }

  @lazyProp
  get immutable(): boolean {
    return this.bindingAsset.getImmutable();
  }

  @lazyProp
  get originalFileName(): string | null {
    return this.bindingAsset.getOriginalFileName() || null;
  }

  @lazyProp
  get originalFileNames(): string[] {
    return this.bindingAsset.getOriginalFileNames();
  }

  @lazyProp
  get name(): string | undefined {
    return this.bindingAsset.getName() ?? undefined;
  }

  @lazyProp
  get names(): string[] {
    return this.bindingAsset.getNames();
  }

  @lazyProp
  get source(): AssetSource {
    return transformAssetSource(this.bindingAsset.getSource());
  }
}
