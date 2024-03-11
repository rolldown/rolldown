import type { RollupOptions, RollupOutput } from 'rolldown'

const config: RollupOptions = {
  external: [/external/, 'external-a'],
}

export default {
  config,
}
