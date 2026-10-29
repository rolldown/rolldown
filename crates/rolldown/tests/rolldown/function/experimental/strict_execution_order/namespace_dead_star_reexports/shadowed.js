function mark() {
  globalThis.shadowedValue =
    typeof globalThis.shadowedValue === 'number' ? globalThis.shadowedValue + 1 : 0;
  return 2;
}

export const x = /* @__PURE__ */ mark();
