function mark() {
  globalThis.defaultValue =
    typeof globalThis.defaultValue === 'number' ? globalThis.defaultValue + 1 : 0;
  return 42;
}

export default /* @__PURE__ */ mark();
