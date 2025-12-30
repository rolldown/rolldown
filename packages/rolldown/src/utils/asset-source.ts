import type { BindingAssetSource } from '../../dist/binding.cjs';

export type AssetSource = string | Uint8Array;

export function transformAssetSource(bindingAssetSource: BindingAssetSource): AssetSource {
  return bindingAssetSource.inner;
}

export function bindingAssetSource(source: AssetSource): BindingAssetSource {
  return {
    inner: source,
  };
}
