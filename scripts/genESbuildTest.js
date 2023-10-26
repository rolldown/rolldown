const Parser = require("tree-sitter");
const Go = require("tree-sitter-go");
const fs = require("fs-extra");
const path = require("path");
const changeCase = require("change-case");
const chalk = require("chalk");
const dedent = require("dedent");
// How to use this script
// 1. Adding a test golang file under this dir or whereever you want, and modify the source path
// 2. Modifying the includeList, this list control which test case you want to generate
// 3. `let testDir = path.resolve(__dirname, "test", testCaseName);` Modify this testDir, by default, 
// The script will generate testCases under `${__dirname}/test`

let source = fs
	.readFileSync(path.resolve(__dirname, "./bundler_default.go"))
	.toString();
const includeList = [
	"avoid_tdz",
	"common_js_from_es6",
	"es6_from_common_js",
	"export_chain",
	"export_forms_es6",
	"export_froms_common_js",
	"nested_common_js",
	"nested_es6_from_common_js",
	"new_expression_common_js",
	"require_child_dir_common_js",
	"require_child_dir_es6",
	"require_parent_dir_common_js",
	"require_parent_dir_es6",
	"simple_common_js",
	"simple_es6",
	"export_forms_common_js",
];
const parser = new Parser();
parser.setLanguage(Go);

const tree = parser.parse(source);

let queryString = `
(call_expression
      arguments: (argument_list 
        ((identifier) @first_param (#eq? @first_param "t")) 
	    (composite_literal
        type: (type_identifier)
        (literal_value
          (keyed_element) @element_list
        )
      )
      )
)
`;

let query = new Parser.Query(parser.getLanguage(), queryString);

for (let i = 0, len = tree.rootNode.namedChildren.length; i < len; i++) {
	let child = tree.rootNode.namedChild(i);
	if (child.type == "function_declaration") {
		let testCaseName = child.namedChild(0).text;
		testCaseName = testCaseName.slice(4);
		testCaseName = changeCase.snakeCase(testCaseName);

		console.log(testCaseName);
		if (!includeList.includes(testCaseName)) {
      // TODO Add a `.` prefix instead of skipping rest of the code.
			continue;
		}
		let bundle_field_list = query.captures(child).filter((item) => {
			return item.name === "element_list";
		});
		let jsConfig = Object.create(null);
		bundle_field_list.forEach((cap) => {
			processKeyElement(cap.node, jsConfig);
		});

		const fileList = jsConfig["files"];
		let prefix = calculatePrefix(fileList.map((item) => item.name));
		let testDir = path.resolve(__dirname, "test", testCaseName);
		fs.ensureDirSync(testDir);
		fileList.forEach((file) => {
			let normalizedName = file.name.slice(prefix.length);
			if (path.isAbsolute(normalizedName)) {
				normalizedName = normalizedName.slice(1);
			}
			const absFile = path.resolve(testDir, normalizedName);
			const dirName = path.dirname(absFile);
			fs.ensureDirSync(dirName);
			fs.writeFileSync(absFile, file.content);
		});

		// entry
		const config = { input: {} };
		const entryPaths = jsConfig["entryPaths"] ?? [];
		if (!entryPaths.length) {
			console.error(chalk.red(`No entryPaths found`));
		}
		let input = entryPaths.map((p) => {
			let normalizedName = p.slice(prefix.length);
			if (path.isAbsolute(normalizedName)) {
				normalizedName = normalizedName.slice(1);
			}
			return {
				name: normalizedName.split("/").join("_").split(".").join("_"),
				import: normalizedName,
			};
		});
		config.input.input = input;
		const configFilePath = path.resolve(testDir, "test.config.json");
		fs.writeFileSync(configFilePath, JSON.stringify(config, null, 2));
		// TODO: options
	}
}

function calculatePrefix(stringList) {
	if (stringList.length < 2) {
		return "";
	}
	let res = "";
	while (true) {
		if (stringList[0][res.length]) {
			res += stringList[0][res.length];
		} else {
			break;
		}
		for (let i = 0; i < stringList.length; i++) {
			if (!stringList[i].startsWith(res)) {
				return res.slice(0, res.length - 1);
			}
		}
	}
	return res;
}

/**
 * @param {import('tree-sitter').SyntaxNode} node
 */
function processFiles(node) {
	let fileList = [];
	let compositeLiteral = node.namedChild(0);
	let body = compositeLiteral.namedChild(1);
	try {
		body.namedChildren.forEach((child) => {
			if (child.type !== "keyed_element") {
				return;
			}
			let name = child.namedChild(0)?.text.slice(1, -1);
			let content = child.namedChild(1).text.slice(1, -1).trim();
			content = dedent.default(content);
			fileList.push({
				name,
				content,
			});
		});
		return fileList;
	} catch (err) {
		console.error(`Error occured when processFiles: ${chalk.red(err)}`);
		return [];
	}
}

/**
 * @param {import('tree-sitter').SyntaxNode} node
 */
function processEntryPath(node) {
	let entryList = [];
	let compositeLiteral = node.namedChild(0);
	let body = compositeLiteral.namedChild(1);
	try {
		body.namedChildren.forEach((child) => {
			let entry = child.namedChild(0).text.slice(1, -1);
			entryList.push(entry);
		});

		return entryList;
	} catch (err) {
		console.error(`Error occured when processEntryPath: ${chalk.red(err)}`);
		return [];
	}
}

/**
 * @param {import('tree-sitter').SyntaxNode} node
 */
function processOptions(node) {}

/**
 * @param {import('tree-sitter').SyntaxNode} node
 */
function processKeyElement(node, obj) {
	let keyValue = node.namedChild(0).text;
	switch (keyValue) {
		case "files":
			obj["files"] = processFiles(node.namedChild(1));
			break;
		case "entryPaths":
			obj["entryPaths"] = processEntryPath(node.namedChild(1));
			break;
		case "options":
			obj["options"] = processOptions(node.namedChild(1));
			break;
		default:
			console.log(chalk.yellow(`unknown filed ${keyValue}`));
			break;
	}
}
