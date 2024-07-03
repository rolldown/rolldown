import "zx/globals";
import { assertRunningScriptFromRepoRoot } from "../../meta/utils.js";
import {
	cloneMultiDuplicatedSymbolsIfNotExists,
	cloneThreeJsIfNotExists,
	fetchRomeIfNotExists,
} from "./util.js";
assertRunningScriptFromRepoRoot();

await cloneThreeJsIfNotExists();
await fetchRomeIfNotExists();
await cloneMultiDuplicatedSymbolsIfNotExists();

await import("./threejs.js");
await import ("./threejs-10x.js");
await import ("./rome.js");
await import ('./multi-duplicated-symbol.js')
