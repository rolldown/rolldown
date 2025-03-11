import { BindingResolvedExternal } from '../binding'
import { PartialResolvedId, ResolvedId } from '../plugin'
import { unimplemented } from './misc'

export function transformResolvedExternal(
  bindingResolvedExternal: BindingResolvedExternal,
): ResolvedId['external'] {
  switch (bindingResolvedExternal.type) {
    case 'Bool':
      return bindingResolvedExternal.field0

    default:
      unimplemented(`external ${bindingResolvedExternal.type}`)
  }
}

export function bindingResolvedExternal(
  external: PartialResolvedId['external'],
): BindingResolvedExternal | undefined {
  if (typeof external === 'boolean') {
    return { type: 'Bool', field0: external }
  }
}
