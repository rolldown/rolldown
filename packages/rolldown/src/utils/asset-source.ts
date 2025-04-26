import type { BindingAssetSource } from '../binding';

export type AssetSource = string | Uint8Array;

export function transformAssetSource(
  bindingAssetSource: BindingAssetSource,
): AssetSource {
  return bindingAssetSource.inner;
}

export function bindingAssetSource(source: AssetSource): BindingAssetSource {
  return {
    inner: source,
  };
}
