function mark() {
  globalThis.valueA = typeof globalThis.valueA === 'number' ? globalThis.valueA + 1 : 0;
  return 'a';
}

export const collision = /* @__PURE__ */ mark();
