export type MaybePromise<T> = T | Promise<T>

export interface AnyFn {
  (...args: any[]): any
}

export interface AnyObj {}

export type NullValue<T = void> = T | undefined | null | void
