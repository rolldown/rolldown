import type { OutputOptions, RolldownOptions, RolldownOutput } from 'rolldown'

export type WithoutValue = 0
type OutputOptsToOutputInner<OutputOpts extends undefined | OutputOptions | OutputOptions[]> =
  OutputOpts extends OutputOptions[]
    ? OutputOpts extends undefined | OutputOptions
      ? RolldownOutput[] | RolldownOutput
      : RolldownOutput[]
    : RolldownOutput
type OutputOptsToOutput<OutputOpts extends WithoutValue | undefined | OutputOptions | OutputOptions[]> =
  [WithoutValue] extends [OutputOpts]
    ? RolldownOutput
    : OutputOptsToOutputInner<Exclude<OutputOpts, WithoutValue>>

export interface TestConfig<OutputOpts extends WithoutValue | undefined | OutputOptions | OutputOptions[] = undefined | OutputOptions | OutputOptions[]> {
  skip?: boolean
  config?: RolldownOptions & { output?: OutputOpts }
  beforeTest?: () => Promise<void> | void
  afterTest?: (output: OutputOptsToOutput<OutputOpts>) => Promise<void> | void
  catchError?: (err: unknown) => Promise<void> | void
}

export type { Plugin } from 'rolldown'
