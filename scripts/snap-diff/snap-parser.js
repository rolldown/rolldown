import { trimStart } from "lodash-es";
import { snakeCase } from "change-case";
import markdown from "markdown-it";
import assert from 'assert'
/**
 * @param {string} source
 *
 **/
export function parseEsbuildSnap(source) {
	let cases = source.split(
		"================================================================================",
	);
	return cases.map(parseEsbuildCase);
}

/**
 * @param {string} source
 * @returns {{name: string, source: string}}
 * */
function parseEsbuildCase(source) {
	let lines = source.trimStart().split("\n");
	let [name, ...rest] = lines;
	let normalizedName = snakeCase(trimStart(name, "Test"));
	return { name: normalizedName, source: rest.join("\n") };
};

const rolldownSnap = `
---
source: crates/rolldown_testing/src/integration_test.rs
---
# Assets

## entry_js.mjs

\`\`\`js
import { default as assert } from "node:assert";

//#region b.js
let x = 1;

//#endregion
//#region entry.js
assert.equal(x, 1);

//#endregion
//

\`\`\`

## test/tes/test/test/someentry.mjs
\`\`\`js
console.log('testj')


\`\`\`
`;
/**
 * @param {string} source
 *
 */
export function parseRolldownSnap(source) {
	let match;
  // strip `---source---` block
	while ((match = /---\n([\s\S]+?)\n---/.exec(source))) {
		source = source.slice(match.index + match[0].length);
	}
	// default mode
	const md = markdown();
	let tokens = md.parse(source);
	let i = 0;
	let ret = [];
	while (i < tokens.length) {
		let token = tokens[i];

		if (token.type === "heading_open" && token.tag === "h2") {
			let headingToken = tokens[i + 1];
			assert(headingToken.type === "inline");
			let filename = headingToken.content;
			let codeToken = tokens[i + 3];
			assert(codeToken.tag === "code");
			let content = codeToken.content;
			ret.push({ filename, content });
			i += 3;
		}
		i++;
	}
  return ret

}

import * as fs from "fs";
import * as path from "path";

const file = fs.readFileSync(
	path.resolve(
		import.meta.dirname,
		"./esbuild-snapshots/snapshots_importstar.txt",
	),
	"utf-8",
);
console.log(`parse(file): `, parseEsbuildSnap(file));
console.log(`parseRolldownSnap(rolldownSnap): `, parseRolldownSnap(rolldownSnap))
