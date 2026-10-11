export const common = 'common';

globalThis.value = typeof globalThis.value === 'number' ? globalThis.value + 1 : 0;

export const _ = () => undefined;
