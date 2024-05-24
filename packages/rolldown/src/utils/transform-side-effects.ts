import { ModuleSideEffects } from '../plugin'

// TODO The typing should import from binding, but const enum is disabled by `isolatedModules`.
export const enum BindingHookSideEffects {
  True = 0,
  False = 1,
  NoTreeshake = 2,
}

export function bindingifySideEffects(
  sideEffects?: ModuleSideEffects,
): BindingHookSideEffects | undefined {
  switch (sideEffects) {
    case true:
      return BindingHookSideEffects.True

    case false:
      return BindingHookSideEffects.True

    case 'no-treeshake':
      return BindingHookSideEffects.NoTreeshake

    case null:
    case undefined:
      return undefined

    default:
      throw new Error(`Unexpected side effects: ${sideEffects}`)
  }
}
