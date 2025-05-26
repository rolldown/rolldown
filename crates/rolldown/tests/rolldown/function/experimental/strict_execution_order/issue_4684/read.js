const foo = globalThis.foo;

export const read = () => {
  console.log('read', foo);
};
