import * as v from 'valibot'

export const stringOrRegExp = (): v.UnionSchema<
  [v.StringSchema<undefined>, v.InstanceSchema<RegExpConstructor, undefined>],
  undefined
> => v.union([v.string(), v.instance(RegExp)])

export const optionalStringArray = (): v.OptionalSchema<
  v.ArraySchema<v.StringSchema<undefined>, undefined>,
  undefined
> => v.optional(v.array(v.string()))

/**
 * We use this to ensure the type of a value is `T` but the value is not checked.
 */
export const phantom = <T>(): v.CustomSchema<T, undefined> =>
  v.custom<T>(() => true)

/**
 * @description Shortcut for `T | null | undefined | void`
 */
export const voidNullable = (): v.UnionSchema<
  [
    v.VoidSchema<undefined>,
    v.NullSchema<undefined>,
    v.UndefinedSchema<undefined>,
  ],
  undefined
> => {
  return v.union([v.void(), v.null(), v.undefined()])
}

/**
 * @description Shortcut for `T | null | undefined | void`
 */
export const voidNullableWith = <
  const TWrapped extends v.BaseSchema<unknown, unknown, v.BaseIssue<unknown>>,
>(
  t: TWrapped,
): v.UnionSchema<
  [
    v.UnionSchema<
      [
        v.VoidSchema<undefined>,
        v.NullSchema<undefined>,
        v.UndefinedSchema<undefined>,
      ],
      undefined
    >,
    any,
  ],
  undefined
> => {
  return v.union([voidNullable(), t])
}
