interface ModuleSideEffectsRule {
  test?: RegExp;
  external?: boolean;
  sideEffects: boolean;
}

type ModuleSideEffectsOption =
  | boolean
  | ModuleSideEffectsRule[]
  | ((id: string, isResolved: boolean) => boolean | undefined)
  | 'no-external';

export type TreeshakingOptions =
  | {
    moduleSideEffects?: ModuleSideEffectsOption;
    annotations?: boolean;
    manualPureFunctions?: string[];
    unknownGlobalSideEffects?: boolean;
  }
  | boolean;
