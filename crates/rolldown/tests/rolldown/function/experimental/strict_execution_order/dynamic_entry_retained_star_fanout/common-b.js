function init() {
  globalThis.valueB = typeof globalThis.valueB === 'number' ? globalThis.valueB + 1 : 0;
}

export const commonB = /* @__PURE__ */ init();
