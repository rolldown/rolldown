import 'zx/globals'
import { assertRunningScriptFromRepoRoot } from '../../meta/utils.js'
import { cloneThreeJsIfNotExists, fetchRomeIfNotExists } from './util.js'
assertRunningScriptFromRepoRoot()

await cloneThreeJsIfNotExists()
await fetchRomeIfNotExists()

await import('./threejs.js')
await import('./threejs-10x.js')
await import('./rome.js')
