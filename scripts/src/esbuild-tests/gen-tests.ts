// To use this script, run:
//    pnpm run gen-esbuild-tests <test-suite-name>

import chalk from 'chalk';
import * as changeCase from 'change-case';
import * as dedent from 'dedent';
import fs from 'fs-extra';
import * as nodeFs from 'node:fs';
import * as fsp from 'node:fs/promises';
import * as nodeHttps from 'node:https';
import * as path from 'node:path';
import {
  Language,
  type Node as SyntaxNode,
  Parser,
  Query,
} from 'web-tree-sitter';
// import Go from 'tree-sitter-go';
import { ESBUILD_BUNDLER_TESTS_URL } from './urls.js';

const TREE_SITTER_WASM_GO_FILENAME = path.resolve(
  import.meta.dirname,
  '../../tmp/tree-sitter-go.wasm',
);

const GO_FILES_DIR = path.resolve(
  import.meta.dirname,
  '../../tmp/esbuild-tests',
);

/**
 * Each test suite is represented by a key-value pair where the key is the name of the test suite,
 * and the value is an object with properties describing the test suite.
 */
const suites = {
  default: {
    name: 'default',
    sourceFile: 'bundler_default_test.go',
    ignoreCases: [],
  },
  dce: {
    name: 'dce',
    sourceFile: 'bundler_dce_test.go',
  },
  importstar: {
    name: 'importstar',
    sourceFile: 'bundler_importstar_test.go',
  },
  importstar_ts: {
    name: 'importstar_ts',
    sourceFile: 'bundler_importstar_ts_test.go',
  },
  ts: {
    name: 'bundler_ts',
    sourceFile: 'bundler_ts_test.go',
  },
  lower: {
    name: 'lower',
    sourceFile: 'bundler_lower_test.go',
  },
  loader: {
    name: 'loader',
    sourceFile: 'bundler_loader_test.go',
  },
  splitting: {
    name: 'splitting',
    sourceFile: 'bundler_splitting_test.go',
  },
  glob: {
    name: 'glob',
    sourceFile: 'bundler_glob_test.go',
  },
} as const satisfies Record<string, {
  name: string;
  sourceFile: string;
  ignoreCases?: string[];
}>;

type TestSuiteName = keyof typeof suites;

interface FileEntry {
  name: string;
  content: string;
}

interface JsConfig {
  files: FileEntry[];
  entryPaths: string[];
  options: void;
  expectedCompileLog?: string;
  expectedScanLog?: string;
}

interface InputEntry {
  name: string;
  import: string;
}

interface Config {
  config: {
    input: InputEntry[];
  };
}

if (process.argv.length < 3) {
  throw new Error(
    `Please provide the test suite name: ${Object.keys(suites).join(', ')}`,
  );
}

const SUITE_NAME = process.argv[2] as TestSuiteName;
console.log(`Processing test suite: ${SUITE_NAME}`);

const __dirname = import.meta.dirname;

const TESTS_ROOT_DIR = path.resolve(
  __dirname,
  '../../../crates/rolldown/tests/esbuild',
  SUITE_NAME,
);

const queryString = `
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
 * Attempts to read the .go source file based on the provided test suite name. {@link suites}
 * Downloads the file if it doesn't exist locally.
 * @param testSuiteName - The name of the current test suite.
 * @returns The contents of the .go test source file.
 *
 * ## Panics
 * Performs {@link process.exit} if it cannot find (and then download) .go source file based on test suite name {@link suites}
 */
async function readTestSuiteSource(
  testSuiteName: TestSuiteName,
): Promise<string> {
  const testSuite = suites[testSuiteName];
  const sourcePath = path.join(GO_FILES_DIR, testSuite.sourceFile);
  const sourceGithubUrl =
    `${ESBUILD_BUNDLER_TESTS_URL}/${testSuite.sourceFile}`;

  try {
    return fs.readFileSync(sourcePath).toString();
  } catch {
    console.log(`Could not read .go source file from ${sourcePath}.`);
    console.log(`Attempting to download it from ${sourceGithubUrl}.`);
    console.log('...');

    try {
      const response = await fetch(sourceGithubUrl);
      const text = await response.text();
      if (typeof text === 'string') {
        await fsp.mkdir(GO_FILES_DIR, { recursive: true });
        await fsp.writeFile(sourcePath, text);
        console.log(`Downloaded and saved at ${sourcePath}.`);
        return fs.readFileSync(sourcePath).toString();
      } else {
        throw new Error('Unexpected shape of source file');
      }
    } catch (err2) {
      console.log(
        'Could not download .go source file. Please download it manually.',
        err2,
      );
      console.log(`Download link: ${sourceGithubUrl}`);
      process.exit(1);
    }
  }
}

function getTopLevelBinding(root: SyntaxNode): Record<string, SyntaxNode> {
  const binding: Record<string, SyntaxNode> = {};
  root.namedChildren.forEach((child) => {
    if (child!.type === 'var_declaration') {
      const var_spec = child!.namedChildren[0]!;
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

function isDirEmptySync(dir: string): boolean {
  const list = fs.readdirSync(dir);
  return list.length === 0;
}

function calculatePrefixDir(paths: string[]): string {
  if (paths.length <= 1) {
    return '';
  }

  const pathComponents = paths.map((p) => p.split('/'));
  let commonPrefix = pathComponents[0];

  for (let i = 1; i < pathComponents.length; i++) {
    for (let j = 0; j < commonPrefix.length; j++) {
      if (pathComponents[i][j] !== commonPrefix[j]) {
        commonPrefix = commonPrefix.slice(0, j);
        break;
      }
    }
  }

  return commonPrefix.join('/');
}

function extractStringLiteral(node: SyntaxNode | null | undefined): string {
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

function processFiles(
  node: SyntaxNode,
  binding: Record<string, SyntaxNode>,
): FileEntry[] {
  if (node.firstChild?.type === 'identifier') {
    const name = node.firstChild.text;
    if (binding[name]) {
      node = binding[name];
    }
  }
  const fileList: FileEntry[] = [];
  const compositeLiteral = node.namedChild(0);
  const body = compositeLiteral?.namedChild(1);
  try {
    if (!body) {
      throw new Error('No body');
    }
    body.namedChildren.forEach((child) => {
      if (child!.type !== 'keyed_element') {
        return;
      }
      const name = child!.namedChild(0)?.text.slice(1, -1);
      if (!name) {
        throw new Error(`File has no name`);
      }
      let content = extractStringLiteral(child!.namedChild(1)?.namedChild?.(0));
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

function processEntryPath(
  node: SyntaxNode,
  binding: Record<string, SyntaxNode>,
): string[] {
  if (node.firstChild?.type === 'identifier') {
    const name = node.firstChild.text;
    if (binding[name]) {
      node = binding[name];
    }
  }
  const entryList: string[] = [];
  const compositeLiteral = node.namedChild(0);
  const body = compositeLiteral?.namedChild(1);
  try {
    if (!body) {
      throw new Error('No body');
    }
    body.namedChildren.forEach((child) => {
      const entry = child!.namedChild(0)?.text.slice(1, -1);
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

// TODO: only preserve mode ModeBundle test case
function processOptions(_node: SyntaxNode): void {}

function processKeyElement(
  node: SyntaxNode,
  jsConfig: JsConfig,
  binding: Record<string, SyntaxNode>,
): void {
  const keyValue = node.namedChild(0)?.text;
  const child = node.namedChild(1);
  if (!child) {
    throw new Error(`Could not find namedChild(1)`);
  }
  switch (keyValue) {
    case 'files':
      jsConfig.files = processFiles(child, binding);
      break;
    case 'entryPaths':
      jsConfig.entryPaths = processEntryPath(child, binding);
      break;
    case 'options':
      jsConfig.options = processOptions(child);
      break;
    case 'expectedCompileLog':
      jsConfig.expectedCompileLog = child.text.slice(1, -1);
      break;
    case 'expectedScanLog':
      jsConfig.expectedScanLog = child.text.slice(1, -1);
      break;
    case 'debugLogs':
      // ignore
      break;
    default:
      console.log(chalk.yellow(`unknown field ${keyValue}`));
      break;
  }
}

function ensureTreeSitterWasmGo(): Promise<void> | undefined {
  if (nodeFs.existsSync(TREE_SITTER_WASM_GO_FILENAME)) {
    return;
  }
  fs.ensureDirSync(path.dirname(TREE_SITTER_WASM_GO_FILENAME));
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

// Main execution
const source = await readTestSuiteSource(SUITE_NAME);

await Parser.init();
await ensureTreeSitterWasmGo();
const Lang = await Language.load(TREE_SITTER_WASM_GO_FILENAME);
const parser = new Parser();
parser.setLanguage(Lang);
const tree = parser.parse(source)!;
const topLevelBindingMap = getTopLevelBinding(tree.rootNode);
const query = new Query(Lang, queryString);

for (let i = 0, len = tree.rootNode.namedChildren.length; i < len; i++) {
  const child = tree.rootNode.namedChild(i);
  if (child?.type === 'function_declaration') {
    let testCaseName = child.namedChild(0)?.text;
    if (!testCaseName) {
      console.error(`No test case name, root's child index: ${i}`);
      continue;
    }
    testCaseName = testCaseName.slice(4); // every function starts with "Test"
    testCaseName = changeCase.snakeCase(testCaseName);

    console.log('testCaseName: ', testCaseName);

    const bundle_field_list = query.captures(child).filter((item) => {
      return item.name === 'element_list';
    });
    const jsConfig: JsConfig = Object.create(null);
    bundle_field_list.forEach((cap) => {
      processKeyElement(cap.node, jsConfig, topLevelBindingMap);
    });

    const fileList = jsConfig.files;

    const testDir = path.resolve(TESTS_ROOT_DIR, testCaseName);
    const ignoredTestDir = path.resolve(TESTS_ROOT_DIR, `.${testCaseName}`);

    if (
      (fs.existsSync(testDir) && !isDirEmptySync(testDir)) ||
      (fs.existsSync(ignoredTestDir) && !isDirEmptySync(ignoredTestDir))
    ) {
      continue;
    } else {
      fs.ensureDirSync(testDir);
    }
    const prefix = calculatePrefixDir(fileList.map((item) => item.name));
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
    const config: Config = { config: Object.create({}) };
    let entryPaths = jsConfig.entryPaths ?? [];
    if (!entryPaths.length) {
      console.error(chalk.red(`No entryPaths found`));
    }
    if (entryPaths.length === 1 && entryPaths[0] === '/*') {
      entryPaths = fileList.map((item) => item.name);
    }
    const input = entryPaths.map((p) => {
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

    const compileLog = jsConfig.expectedCompileLog;
    if (compileLog) {
      const compileLogPath = path.resolve(testDir, 'compile-log.txt');
      fs.writeFileSync(compileLogPath, compileLog);
    }
    const scanLog = jsConfig.expectedScanLog;
    if (scanLog) {
      const scanLogPath = path.resolve(testDir, 'scan-log.txt');
      fs.writeFileSync(scanLogPath, scanLog);
    }
  }
}
