export type MaybePromise<T> = T | Promise<T>;

export interface AnyFn {
  (...args: any[]): any;
}

interface AnyObj {}

export type NullValue<T = void> = T | undefined | null | void;

export type PartialNull<T> = {
  [P in keyof T]: T[P] | null;
};

export type MakeAsync<Function_> = Function_ extends (
  this: infer This,
  ...parameters: infer Arguments
) => infer Return
  ? (this: This, ...parameters: Arguments) => Return | Promise<Return>
  : never;

export type MaybeArray<T> = T | T[];

export type StringOrRegExp = string | RegExp;
