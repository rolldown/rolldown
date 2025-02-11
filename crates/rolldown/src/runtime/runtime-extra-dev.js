/**
 * This object contains the runtime helpers for the development environment.
 */
export const __runtime = {
  registerModule(id, exports) {
    const modules = self.__modules || (self.__modules = {});
    modules[id] = {
      exports,
    };
  }
}

self.__rolldown_runtime__ = __runtime;