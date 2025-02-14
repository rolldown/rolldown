import { shared } from 'test-shared'
import { sharedCjs } from 'test-shared-cjs'

export const value = 'expose'

export const exposeShared = shared

export const exposeSharedCjs = sharedCjs

export default 'expose-default'
