globalThis.defaultValue =
  typeof globalThis.defaultValue === 'number' ? globalThis.defaultValue + 1 : 0;

export default () => 42;
