import { BindingResolvedExternal } from '../binding'
import { PartialResolvedId, ResolvedId } from '../plugin'
import { unreachable } from './misc'

export function transformResolvedExternal(
  bindingResolvedExternal: BindingResolvedExternal,
): ResolvedId['external'] {
  switch (bindingResolvedExternal.type) {
    case 'Bool':
      return bindingResolvedExternal.field0

    case 'Absolute':
      return 'absolute'

    case 'Relative':
      unreachable(
        `The PluginContext resolve result external couldn't be 'relative'`,
      )
  }
}

export function bindingResolvedExternal(
  external: PartialResolvedId['external'],
): BindingResolvedExternal | undefined {
  if (typeof external === 'boolean') {
    return { type: 'Bool', field0: external }
  }
  if (external === 'absolute') {
    return { type: 'Absolute' }
  }
  if (external === 'relative') {
    return { type: 'Relative' }
  }
}
