declare module 'vitest' {
  interface ProvidedContext {
    /** version of the workspace `rolldown` package, supplied by `provide` in vitest.config.mts */
    rolldownVersion: string;
  }
}

export {};
