function mark() {
  globalThis.valueB = typeof globalThis.valueB === 'number' ? globalThis.valueB + 1 : 0;
  return 'b';
}

export const collision = /* @__PURE__ */ mark();
