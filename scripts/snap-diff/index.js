import * as path from "path";
import * as fs from "fs";
import { trimStart } from "lodash-es";

const esbuildTestDir = path.join(
	import.meta.dirname,
	"../../crates/rolldown/tests/esbuild",
);

function main() {
	// parse esbuild snap
	console.log(`getEsbuildSnap(): `, getEsbuildSnap());
}

function getEsbuildSnap() {
	let fileList = fs.readdirSync(
		path.resolve(import.meta.dirname, "./esbuild-snapshots/"),
	);
	return fileList.map((file) => {
    let name = path.parse(file).name;
    let [_, ...normalizedName] = name.split('_')
		return [normalizedName.join('_'), name]
	});
}

main();
