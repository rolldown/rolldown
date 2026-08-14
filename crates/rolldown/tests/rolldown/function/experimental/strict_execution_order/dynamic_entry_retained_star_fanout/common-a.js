function init() {
  globalThis.valueA = typeof globalThis.valueA === 'number' ? globalThis.valueA + 1 : 0;
}

export const commonA = /* @__PURE__ */ init();
