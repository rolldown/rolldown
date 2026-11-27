export namespace A {
  export enum E {
    X = 1,
  }
}

export namespace B {
  const E = { X: 'b' };
  export const value = E.X;
  export const object = E;
}
