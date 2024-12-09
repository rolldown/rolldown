import type Z from 'zod'
import { z } from 'zod'

export const stringOrRegExp = (): Z.ZodUnion<
  [Z.ZodString, Z.ZodType<RegExp, Z.ZodTypeDef, RegExp>]
> => z.string().or(z.instanceof(RegExp))

export const optionalStringArray = (): Z.ZodOptional<
  Z.ZodArray<Z.ZodString, 'many'>
> => z.string().array().optional()

const returnTrue = () => true

/**
 * We use this to ensure the type of a value is `T` but the value is not checked.
 */
export const phantom = <T>(): Z.ZodType<T> => z.custom<T>(returnTrue)

/**
 * @description Shortcut for `T | null | undefined | void`
 */
export const voidNullableWith = <T extends z.ZodTypeAny>(
  t: T,
): Z.ZodUnion<
  [Z.ZodUnion<[Z.ZodUnion<[Z.ZodVoid, Z.ZodNull]>, Z.ZodUndefined]>, T]
> => {
  return voidNullable().or(t)
}

/**
 * @description Shortcut for `T | null | undefined | void`
 */
export const voidNullable = (): Z.ZodUnion<
  [Z.ZodUnion<[Z.ZodVoid, Z.ZodNull]>, Z.ZodUndefined]
> => {
  return z.void().or(z.null()).or(z.undefined())
}
