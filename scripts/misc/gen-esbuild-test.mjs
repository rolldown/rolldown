import { Parser } from 'web-tree-sitter';
// import Go from 'tree-sitter-go'
import chalk from 'chalk';
import * as changeCase from 'change-case';
import * as dedent from 'dedent';
import fs from 'fs-extra';
import * as fsExtra from 'fs-extra';
import * as nodeFs from 'node:fs';
import fsp from 'node:fs/promises';
import * as nodeHttps from 'node:https';
import * as path from 'node:path';

const TREE_SITTER_WASM_GO_FILENAME = path.resolve(
  import.meta.dirname,
  '../../tmp/tree-sitter-go.wasm',
);

// How to use this script
// 1. Set the test suite name.

/** @type {TestSuiteName} {@link suites} */
if (process.argv.length < 3) {
  throw new Error('Please provide the test suite name');
}

const SUITE_NAME = process.argv[2];
console.log(`Processing test suite: ${SUITE_NAME}`);

const __dirname = import.meta.dirname;

// 2. Set the tests root directory

const TESTS_ROOT_DIR = path.resolve(
  __dirname,
  '../../crates/rolldown/tests/esbuild',
  SUITE_NAME,
);

// 3. Download .go test source file located in the suites object
//    for each suite and place it under "scripts" dir.
//    (You can skip this step, the script can download it for you)

/**
 * Constant object containing test suites.
 * Each test suite is represented by a key-value pair where the key is the name of the test suite,
 * and the value is an object with properties describing the test suite.
 * Each test suite includes a link where you can copy the test file.
 * Download the file needed for your test suite and place it under this directory.
 * @readonly
 */
const suites = /** @type {const} */ ({
  default: {
    name: 'default',
    sourcePath: './bundler_default_test.go',
    sourceGithubUrl:
      'https://raw.githubusercontent.com/evanw/esbuild/main/internal/bundler_tests/bundler_default_test.go',
    ignoreCases: [],
  },
  dce: {
    name: 'dce',
    sourcePath: './bundler_dce_test.go',
    sourceGithubUrl:
      'https://raw.githubusercontent.com/evanw/esbuild/main/internal/bundler_tests/bundler_dce_test.go',
  },
  importstar: {
    name: 'importstar',
    sourcePath: './bundler_importstar_test.go',
    sourceGithubUrl:
      'https://raw.githubusercontent.com/evanw/esbuild/main/internal/bundler_tests/bundler_importstar_test.go',
  },
  importstar_ts: {
    name: 'importstar_ts',
    sourcePath: './bundler_importstar_ts_test.go',
    sourceGithubUrl:
      'https://raw.githubusercontent.com/evanw/esbuild/main/internal/bundler_tests/bundler_importstar_ts_test.go',
  },
  ts: {
    name: 'bundler_ts',
    sourcePath: './bundler_ts_test.go',
    sourceGithubUrl:
      'https://raw.githubusercontent.com/evanw/esbuild/main/internal/bundler_tests/bundler_ts_test.go',
  },
  lower: {
    name: 'lower',
    sourcePath: './bundler_lower_test.go',
    sourceGithubUrl:
      'https://raw.githubusercontent.com/evanw/esbuild/main/internal/bundler_tests/bundler_lower_test.go',
  },
  loader: {
    name: 'loader',
    sourcePath: './bundler_loader_test.go',
    sourceGithubUrl:
      'https://raw.githubusercontent.com/evanw/esbuild/main/internal/bundler_tests/bundler_loader_test.go',
  },
  splitting: {
    name: 'splitting',
    sourcePath: './bundler_splitting_test.go',
    sourceGithubUrl:
      'https://raw.githubusercontent.com/evanw/esbuild/main/internal/bundler_tests/bundler_splitting_test.go',
  },
  glob: {
    name: 'glob',
    sourcePath: './bundler_glob_test.go',
    sourceGithubUrl:
      'https://raw.githubusercontent.com/evanw/esbuild/main/internal/bundler_tests/bundler_glob_test.go',
  },
});
/**
 * The key of the suites constant. {@link suites}
 * @typedef {keyof suites} TestSuiteName
 */

/**
 * An object with properties describing the test suite.
 * @typedef {suites[keyof suites]} TestSuite
 */

/** @typedef {{files: Array<{name: string; content: string}>; entryPaths: string[]; options: void; expectedCompileLog?: string}} JsConfig */

/**
 * Attempts to read the .go source file based on the provided test suite name. {@link suites}
 * @param {TestSuiteName} testSuiteName - The name of the current test suite.
 * @returns {Promise<string>} The contents of the .go test source file.
 *
 * ## Panics
 * Performs {@link process.exit} with helpful text error if cannot find(and then download) .go source file based on test suite name {@link suites}
 */
async function readTestSuiteSource(testSuiteName) {
  const testSuite = suites[testSuiteName];
  const sourcePath = path.resolve(__dirname, testSuite.sourcePath);
  try {
    return fs.readFileSync(sourcePath).toString();
  } catch {
    console.log(`Could not read .go source file from ${sourcePath}.`);
    console.log(`Attempting to download it from ${testSuite.sourceGithubUrl}.`);
    console.log('...');

    // download from github
    try {
      const response = await fetch(testSuite.sourceGithubUrl);
      const text = await response.text();
      if (typeof text === 'string') {
        // save under scripts directory
        await fsp.writeFile(sourcePath, text);
        console.log(`Downloaded and saved at ${sourcePath}.`);
        return fs.readFileSync(sourcePath).toString();
      } else {
        throw new Error('Unexpected shape of source file');
      }
    } catch (err2) {
      console.log(
        'Could not download .go source file. Please download it manually and save it under the "scripts" directory.',
        err2,
      );
      console.log(`Download link: ${testSuite.sourceGithubUrl}`);
      process.exit(1);
    }
  }
}

/** The contents of the .go test source file. {@link suites} */
const source = await readTestSuiteSource(SUITE_NAME);

// This is up to suit name
// const ignoreCases = suites[SUITE_NAME]?.ignoreCases ?? []
// Generic ignored pattern, maybe used in many suites
// const ignoredTestPattern = [
//   'ts',
//   'txt',
//   'json',
//   'jsx',
//   'tsx',
//   'no_bundle',
//   'mangle',
//   'minify',
//   'minified',
//   'comments',
//   'fs',
//   'alias',
//   'node',
//   'decorator',
//   'iife',
//   'abs_path',
//   'inject',
//   'metafile',
//   'output_extension',
//   'top_level_return_forbidden',
// ]

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

/**
 * @param {import("web-tree-sitter").SyntaxNode} root
 * @returns {Record<string, Parser.SyntaxNode>}
 */
function getTopLevelBinding(root) {
  /** @type {Record<string, Parser.SyntaxNode>} */
  const binding = {};
  root.namedChildren.forEach((child) => {
    if (child.type === 'var_declaration') {
      const var_spec = child.namedChildren[0];
      const name = var_spec.namedChild(0)?.text;
      const decl = var_spec.namedChild(1);
      if (!name || !decl) {
        return;
      }
      binding[name] = decl;
    }
  });
  return binding;
}

await Parser.init();
await ensureTreeSitterWasmGo();
const Lang = await Parser.Language.load(TREE_SITTER_WASM_GO_FILENAME);
const parser = new Parser();
parser.setLanguage(Lang);
const tree = parser.parse(source);
let topLevelBindingMap = getTopLevelBinding(tree.rootNode);
const query = Lang.query(queryString);

/**
 * @param {string} dir - The directory path.
 * @returns {boolean}
 */
function isDirEmptySync(dir) {
  let list = fs.readdirSync(dir);
  return list.length === 0;
}

for (let i = 0, len = tree.rootNode.namedChildren.length; i < len; i++) {
  let child = tree.rootNode.namedChild(i);
  if (child?.type == 'function_declaration') {
    let testCaseName = child.namedChild(0)?.text;
    if (!testCaseName) {
      console.error(`No test case name, root's child index: ${i}`);
      continue;
    }
    testCaseName = testCaseName.slice(4); // every function starts with "Test"
    testCaseName = changeCase.snakeCase(testCaseName);

    console.log('testCaseName: ', testCaseName);

    // let isIgnored = false
    // Skip some test cases by ignoredTestName
    // if (ignoredTestPattern.some((name) => testCaseName?.includes(name))) {
    //   isIgnored = true
    // }
    // if (ignoreCases.includes(testCaseName)) {
    //   isIgnored = true
    // }
    let bundle_field_list = query.captures(child).filter((item) => {
      return item.name === 'element_list';
    });
    /** @type {JsConfig} */
    let jsConfig = Object.create(null);
    bundle_field_list.forEach((cap) => {
      processKeyElement(cap.node, jsConfig, topLevelBindingMap);
    });

    const fileList = jsConfig['files'];

    const testDir = path.resolve(TESTS_ROOT_DIR, testCaseName);
    const ignoredTestDir = path.resolve(TESTS_ROOT_DIR, `.${testCaseName}`);

    // Cause if you withdraw directory in git system, git will cleanup dir but leave the directory alone
    if (
      (fs.existsSync(testDir) && !isDirEmptySync(testDir)) ||
      (fs.existsSync(ignoredTestDir) && !isDirEmptySync(ignoredTestDir))
    ) {
      continue;
    } else {
      fs.ensureDirSync(testDir);
    }
    let prefix = calculatePrefixDir(fileList.map((item) => item.name));
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
    /** @type {{config: {input: Array<{name: string; import: string}>}}} */
    const config = { config: Object.create({}) };
    let entryPaths = jsConfig['entryPaths'] ?? [];
    if (!entryPaths.length) {
      console.error(chalk.red(`No entryPaths found`));
    }
    if (entryPaths.length === 1 && entryPaths[0] === '/*') {
      entryPaths = fileList.map((item) => item.name);
    }
    let input = entryPaths.map((p) => {
      let normalizedName = p.slice(prefix.length);
      if (path.isAbsolute(normalizedName)) {
        normalizedName = normalizedName.slice(1);
      }
      return {
        name: normalizedName
          .split('/')
          .filter(Boolean)
          .join('_')
          .split('.')
          .join('_'),
        import: normalizedName,
      };
    });
    config.config.input = input;
    const configFilePath = path.resolve(testDir, '_config.json');
    fs.writeFileSync(configFilePath, JSON.stringify(config, null, 2));
    // TODO: options

    let log = jsConfig['expectedCompileLog'];
    if (log) {
      const compileLogPath = path.resolve(testDir, 'compile-log.txt');
      fs.writeFileSync(compileLogPath, log);
    }
  }
}

/**
 * @param {string[]} paths
 * @returns {string}
 */
function calculatePrefixDir(paths) {
  if (paths.length === 1) {
    return '';
  }

  // Split each path into directory components
  const pathComponents = paths.map((path) => path.split('/'));

  // Initialize the common directory prefix with the first path
  let commonPrefix = pathComponents[0];

  // Iterate over each path's components
  for (let i = 1; i < pathComponents.length; i++) {
    // Compare each directory component in the current path with the common prefix
    for (let j = 0; j < commonPrefix.length; j++) {
      if (pathComponents[i][j] !== commonPrefix[j]) {
        // If components don't match, truncate the common prefix
        commonPrefix = commonPrefix.slice(0, j);
        break;
      }
    }
  }

  // Join the common directory components back into a path
  return commonPrefix.join('/');
}

/**
 * @param {Parser.SyntaxNode} node
 * @param {Record<string, Parser.SyntaxNode>} binding
 * @returns {Array<{name: string; content: string}>}
 */
function processFiles(node, binding) {
  if (node.firstChild?.type === 'identifier') {
    let name = node.firstChild.text;
    if (binding[name]) {
      node = binding[name];
    }
  }
  /** @type Array<{name: string; content: string}> */
  let fileList = [];
  let compositeLiteral = node.namedChild(0);
  let body = compositeLiteral?.namedChild(1);
  try {
    if (!body) {
      throw new Error('No body');
    }
    body.namedChildren.forEach((child) => {
      if (child.type !== 'keyed_element') {
        return;
      }
      let name = child.namedChild(0)?.text.slice(1, -1);
      if (!name) {
        throw new Error(`File has no name`);
      }
      let content = extractStringLiteral(child.namedChild(1)?.namedChild?.(0));
      content = dedent.default(content);
      fileList.push({
        name,
        content,
      });
    });
    return fileList;
  } catch (err) {
    console.error(`Error occurred when processFiles: ${chalk.red(err)}`);
    return [];
  }
}

/**
 * @param {Parser.SyntaxNode} node
 */
function extractStringLiteral(node) {
  if (!node) {
    return '';
  }
  let ret = '';
  switch (node.type) {
    case 'binary_expression':
      ret += extractStringLiteral(node.namedChild(0));
      ret += extractStringLiteral(node.namedChild(1));
      break;
    case 'raw_string_literal':
    case 'interpreted_string_literal':
      ret += node.text.slice(1, -1);
      break;
    default:
      throw new Error(`Unexpected node type: ${node.type}`);
  }
  return ret;
}

/**
 * @param {Parser.SyntaxNode} node
 * @param {Record<string, Parser.SyntaxNode>} binding
 * @returns {string[]}
 */
function processEntryPath(node, binding) {
  if (node.firstChild?.type === 'identifier') {
    let name = node.firstChild.text;
    if (binding[name]) {
      node = binding[name];
    }
  }
  /** @type {string[]} */
  let entryList = [];
  let compositeLiteral = node.namedChild(0);
  let body = compositeLiteral?.namedChild(1);
  try {
    if (!body) {
      throw new Error('No body');
    }
    body.namedChildren.forEach((child) => {
      let entry = child.namedChild(0)?.text.slice(1, -1);
      if (!entry) {
        throw new Error('No entry');
      }
      entryList.push(entry);
    });

    return entryList;
  } catch (err) {
    console.error(`Error occurred when processEntryPath: ${chalk.red(err)}`);
    return [];
  }
}

// TODO only preserve mode ModeBundle test case
/**
 * @param {Parser.SyntaxNode} _node
 */
function processOptions(_node) {}

/**
 * @param {Parser.SyntaxNode} node
 * @param {JsConfig} jsConfig
 * @param {Record<string, Parser.SyntaxNode>} binding
 * @returns {void}
 */
function processKeyElement(node, jsConfig, binding) {
  let keyValue = node.namedChild(0)?.text;
  let child = node.namedChild(1);
  if (!child) {
    throw new Error(`Could not find namedChild(1)`);
  }
  switch (keyValue) {
    case 'files':
      jsConfig['files'] = processFiles(child, binding);
      break;
    case 'entryPaths':
      jsConfig['entryPaths'] = processEntryPath(child, binding);
      break;
    case 'options':
      jsConfig['options'] = processOptions(child);
      break;
    case 'expectedCompileLog':
      jsConfig['expectedCompileLog'] = child.text.slice(1, -1);
      break;
    default:
      console.log(chalk.yellow(`unknown filed ${keyValue}`));
      break;
  }
}

function ensureTreeSitterWasmGo() {
  if (nodeFs.existsSync(TREE_SITTER_WASM_GO_FILENAME)) {
    return;
  }
  fsExtra.ensureDirSync(path.dirname(TREE_SITTER_WASM_GO_FILENAME));
  return new Promise((rsl, rej) => {
    nodeHttps.get(
      'https://tree-sitter.github.io/tree-sitter-go.wasm',
      (resp) => {
        resp.on('end', () => {
          console.log('saved', TREE_SITTER_WASM_GO_FILENAME);
          rsl();
        });
        resp.on('error', rej);
        resp.pipe(nodeFs.createWriteStream(TREE_SITTER_WASM_GO_FILENAME));
      },
    );
  });
}
