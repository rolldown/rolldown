export interface TreeshakingOptions {
  moduleSideEffects?: boolean | ModuleSideEffectsRule[] | 'no-external'
}

export type ModuleSideEffectsRule = {
  test?: RegExp
  external?: boolean
  sideEffects: boolean
}

export * from './module-side-effects'
