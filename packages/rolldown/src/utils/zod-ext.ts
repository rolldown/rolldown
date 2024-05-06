import { z } from 'zod'

export const stringOrRegExp = () => z.string().or(z.instanceof(RegExp))

export const optionalStringArray = () => z.string().array().optional()

const returnTrue = () => true

/**
 * We use this to ensure the type of a value is `T` but the value is not checked.
 */
export const phantom = <T>() => z.custom<T>(returnTrue)

/**
 * @description Shortcut for `T | null | undefined | void`
 */
export const voidNullableWith = <T extends z.ZodTypeAny>(t: T) => {
  return voidNullable().or(t)
}

/**
 * @description Shortcut for `T | null | undefined | void`
 */
export const voidNullable = () => {
  return z.void().or(z.null()).or(z.undefined())
}
