export const common = 'common';

globalThis.namespaceValue =
  typeof globalThis.namespaceValue === 'number' ? globalThis.namespaceValue + 1 : 0;

export const _ = () => undefined;
