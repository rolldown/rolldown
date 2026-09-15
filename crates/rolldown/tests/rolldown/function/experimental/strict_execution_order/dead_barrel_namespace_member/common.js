export const common = 'common';

function mark() {
  globalThis.namespaceValue =
    typeof globalThis.namespaceValue === 'number' ? globalThis.namespaceValue + 1 : 0;
}

export const _ = /* @__PURE__ */ mark();
