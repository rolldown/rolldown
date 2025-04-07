export type DebugConfig = {
  debug?: boolean;
  verbose?: boolean;
  caseNames: string[];
};

export type UnwrapPromise<T extends Promise<any>> = T extends Promise<infer U>
  ? U
  : T;
