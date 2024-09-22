// @ts-check
import * as path from "path";
import * as fs from "fs";
import { parseEsbuildSnap } from "./snap-parser.js";
import { functions } from "lodash-es";
const esbuildTestDir = path.join(
	import.meta.dirname,
	"../../crates/rolldown/tests/esbuild",
);

/**
 * @param {string[]} includeList
 * @returns {Array<{normalizedName: string, content: string}>}
 */
export function getEsbuildSnapFile(includeList) {
	let dirname = path.resolve(import.meta.dirname, "./esbuild-snapshots/");
	let fileList = fs.readdirSync(dirname);
	let ret = fileList
		.filter((filename) => {
			return includeList.length === 0 || includeList.includes(filename);
		})
		.map((filename) => {
			let name = path.parse(filename).name;
			let [_, ...rest] = name.split("_");
			let normalizedName = rest.join("_");
			let content = fs.readFileSync(path.join(dirname, filename), "utf-8");
			return { normalizedName, content };
		});
	return ret;
}

/**
 * @param {string[]} includeList
 */
export function run(includeList) {
	let snapfileList = getEsbuildSnapFile(includeList);
  // snapshot_x.txt
	for (let snapFile of snapfileList) {
		let { normalizedName: snapType, content } = snapFile;
		let parsedEsbuildSnap = parseEsbuildSnap(content);
    // singleSnapshot
    for (let snap of parsedEsbuildSnap) {
      let rolldownTestPath  = path.join(esbuildTestDir, snapType, snap.name)
      console.log(`testRelativePath: `, rolldownTestPath)
    }
	}
}

function getRolldownSnap() {

}
