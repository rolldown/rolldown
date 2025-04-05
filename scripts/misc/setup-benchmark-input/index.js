import 'zx/globals';
import { assertRunningScriptFromRepoRoot } from '../../meta/utils.js';
import {
  cloneRolldownBenchcasesIfNotExists,
  cloneThreeJsIfNotExists,
  fetchRomeIfNotExists,
} from './util.js';
assertRunningScriptFromRepoRoot();

await cloneThreeJsIfNotExists();
await fetchRomeIfNotExists();
await cloneRolldownBenchcasesIfNotExists();

await import('./threejs.js');
await import('./threejs-10x.js');
await import('./rome.js');
await import('./rolldown-benchcases.js');
await import('./antd.js');
