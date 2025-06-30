import type { OutputOptions } from 'rolldown'
import type { TestConfig, WithoutValue } from './types'

export function defineTest<OutputOpts extends WithoutValue | undefined | OutputOptions | OutputOptions[]>(
  testConfig: TestConfig<OutputOpts>
): TestConfig<OutputOpts> {
  return testConfig
}
