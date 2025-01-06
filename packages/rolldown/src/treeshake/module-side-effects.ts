export interface ModuleSideEffectsRule {
  test?: RegExp
  external?: boolean
  sideEffects: boolean
}

export type ModuleSideEffectsOption =
  | boolean
  | ModuleSideEffectsRule[]
  | ((id: string, isResolved: boolean) => boolean | undefined)
  | 'no-external'

export type TreeshakingOptions =
  | {
      moduleSideEffects?: ModuleSideEffectsOption
      annotations?: boolean
    }
  | boolean
