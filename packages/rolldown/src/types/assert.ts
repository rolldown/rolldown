export type TypeAssert<T extends true> = T;

export type HasProperty<T, K extends string> = K extends keyof T ? true : false;

export type Extends<A, B> = A extends B ? true : false;

export type AssertNever<T extends never> = T;
