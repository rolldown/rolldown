import type { RollupOptions } from '../../../../src'

const config: RollupOptions = {
  external: [/external/, 'external-a'],
}

export default {
  config,
}
