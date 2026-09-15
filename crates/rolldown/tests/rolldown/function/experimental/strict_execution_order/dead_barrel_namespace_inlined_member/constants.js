export const x = 1;

function mark() {
  globalThis.inlineNamespaceValue = 0;
}

export const y = /* @__PURE__ */ mark();
