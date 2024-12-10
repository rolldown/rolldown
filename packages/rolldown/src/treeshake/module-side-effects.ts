import * as v from 'valibot'

export interface ModuleSideEffectsRule {
  test?: RegExp
  external?: boolean
  sideEffects: boolean
}

export const ModuleSideEffectsRuleSchema: v.GenericSchema<ModuleSideEffectsRule> =
  v.pipe(
    v.object({
      test: v.optional(v.instance(RegExp)),
      external: v.optional(v.boolean()),
      sideEffects: v.boolean(),
    }),
    v.check((data) => {
      return data.test !== undefined || data.external !== undefined
    }, 'Either `test` or `external` should be set.'),
  )

export type ModuleSideEffectsOption =
  | boolean
  | ModuleSideEffectsRule[]
  | ((id: string, isResolved: boolean) => boolean | undefined)
  | 'no-external'

export const ModuleSideEffectsOptionSchema: v.UnionSchema<
  [
    v.BooleanSchema<undefined>,
    v.LiteralSchema<'no-external', undefined>,
    v.ArraySchema<
      v.GenericSchema<
        ModuleSideEffectsRule,
        ModuleSideEffectsRule,
        v.BaseIssue<unknown>
      >,
      undefined
    >,
    v.SchemaWithPipe<
      [
        v.FunctionSchema<undefined>,
        v.ArgsAction<
          (...args: unknown[]) => unknown,
          v.TupleSchema<
            [
              v.SchemaWithPipe<
                [v.StringSchema<undefined>, v.DecimalAction<string, undefined>]
              >,
            ],
            undefined
          >
        >,
      ]
    >,
  ],
  undefined
> = v.union([
  v.boolean(),
  v.literal('no-external'),
  v.array(ModuleSideEffectsRuleSchema),
  v.pipe(v.function(), v.args(v.tuple([v.pipe(v.string(), v.decimal())]))),
])

export type TreeshakingOptions =
  | {
      moduleSideEffects?: ModuleSideEffectsOption
      annotations?: boolean
    }
  | boolean

export const TreeshakingOptionsSchema: v.UnionSchema<
  [
    v.BooleanSchema<undefined>,
    v.LooseObjectSchema<
      {
        readonly annotations: v.OptionalSchema<
          v.BooleanSchema<undefined>,
          undefined
        >
      },
      undefined
    >,
  ],
  undefined
> = v.union([
  v.boolean(),
  v.looseObject({ annotations: v.optional(v.boolean()) }),
])
