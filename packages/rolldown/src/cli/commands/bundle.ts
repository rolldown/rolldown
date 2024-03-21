import { performance } from 'node:perf_hooks'
import consola from 'consola'
import { colors } from 'consola/utils'
import { RolldownOptions, rolldown } from '../../index'
import { RolldownConfigExport } from '../../types/rolldown-config-export'
import { arraify } from '../../utils'

export async function bundle(configExport: RolldownConfigExport) {
  const options = arraify(configExport)

  for (const option of options) {
    await bundleInner(option)
  }
}

async function bundleInner(options: RolldownOptions) {
  const start = performance.now()

  const build = await rolldown(options)

  const _output = await build.write(options?.output)

  consola.log(
    `Finished in ${colors.bold((performance.now() - start).toFixed(2))} ms`,
  )
}
