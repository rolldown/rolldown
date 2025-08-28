interface ModuleSideEffectsRule {
  test?: RegExp;
  external?: boolean;
  sideEffects: boolean;
}

type ModuleSideEffectsOption =
  | boolean
  | readonly string[]
  | ModuleSideEffectsRule[]
  | ((id: string, external: boolean) => boolean | undefined)
  | 'no-external';

export type TreeshakingOptions = {
  moduleSideEffects?: ModuleSideEffectsOption;
  annotations?: boolean;
  manualPureFunctions?: readonly string[];
  unknownGlobalSideEffects?: boolean;
  commonjs?: boolean;
  propertyReadSideEffects?: false | 'always';
};
