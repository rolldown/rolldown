const __modules = globalThis.__modules || (globalThis.__modules = {});

/**
 * This object contains the runtime helpers for the development environment.
 */
export const __runtime = {
  registerModule(id, exports) {
    __modules[id] = {
      exports,
    };
  },
  modules: __modules,
}

globalThis.__rolldown_runtime__ = __runtime;