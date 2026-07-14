export const common = 'common';

function init() {
  globalThis.value = typeof globalThis.value === 'number' ? globalThis.value + 1 : 0;
}

export const _ = /* @__PURE__ */ init();
