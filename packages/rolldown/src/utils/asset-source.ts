import { BindingAssetSource } from '../binding'
import { Buffer } from 'node:buffer'

export type AssetSource = string | Uint8Array

export function transformAssetSource(
  bindingAssetSource: BindingAssetSource,
): AssetSource {
  if (bindingAssetSource.type === 'string') {
    return bindingAssetSource.source.toString()
  } else {
    return bindingAssetSource.source
  }
}

export function bindingAssetSource(source: AssetSource): BindingAssetSource {
  if (typeof source === 'string') {
    return {
      type: 'string',
      source: Buffer.from(source, 'utf-8'),
    }
  } else {
    return {
      type: 'buffer',
      source,
    }
  }
}
