import type { IsEqual } from 'type-fest';

export type TypeAssert<T extends true> = T;

export type HasProperty<T, K extends string> = K extends keyof T ? true : false;

export type IsPropertyEqual<A, B, Key extends keyof A & keyof B> = IsEqual<
  A[Key],
  B[Key]
>;

type IsValuesOfObjectAllTrue<T> = {
  [K in keyof T]: T[K] extends true ? true : false;
}[keyof T] extends true ? true
  : false;

export type ShowPropertiesEqualStatus<A, B> = {
  // If `K` only exists in `A`, we consider they are equal.
  [K in keyof A]: K extends keyof B ? IsEqual<A[K], B[K]> : true;
};

export type IsPropertiesEqual<A, B> = IsValuesOfObjectAllTrue<
  ShowPropertiesEqualStatus<A, B>
>;

export type Extends<A, B> = A extends B ? true : false;

export type AssertNever<T extends never> = T;
