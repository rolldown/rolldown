globalThis.shadowedValue =
  typeof globalThis.shadowedValue === 'number' ? globalThis.shadowedValue + 1 : 0;

export const x = () => 2;
