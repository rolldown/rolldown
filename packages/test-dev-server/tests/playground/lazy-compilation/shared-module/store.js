globalThis.__sharedModuleStoreRuns = (globalThis.__sharedModuleStoreRuns ?? 0) + 1;

export const store = { run: globalThis.__sharedModuleStoreRuns };
