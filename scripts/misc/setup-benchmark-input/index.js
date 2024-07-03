import 'zx/globals'
import { assertRunningScriptFromRepoRoot } from '../../meta/utils.js'
import { cloneThreeJsIfNotExists, fetchRomeIfNotExists } from './util.js'
assertRunningScriptFromRepoRoot()

await cloneThreeJsIfNotExists()
await fetchRomeIfNotExists()

import './threejs.js'
import './threejs-10x.js'
import './rome.js'
