// @ts-nocheck
import Parser from 'tree-sitter'
import Go from 'tree-sitter-go'
import fs from 'fs-extra'
import path from 'node:path'
import * as changeCase from 'change-case'
import chalk from 'chalk'
import * as dedent from 'dedent'

const __dirname = import.meta.dirname

// How to use this script
// 1. Adding a test golang file under this dir or wherever you want, and modify the source path
// 2. `let testDir = path.resolve(__dirname, "test", testCaseName);` Modify this testDir, by default,
// The script will generate testCases under `${__dirname}/test`

/**
 * Constant object containing test cases.
 * Each test case is represented by a key-value pair where the key is the name of the test case,
 * and the value is an object with properties describing the test case.
 * Each test case includes a comment with the link where you can copy the test file.
 * Download the file needed for your test case and place it under this directory.
 * @readonly
 */
const cases = /** @type {const} */ ({
  /** https://github.com/evanw/esbuild/blob/main/internal/bundler_tests/bundler_default_test.go */
  default: {
    name: 'default',
    sourcePath: './bundler_default_test.go',
  },
  /** https://github.com/evanw/esbuild/blob/main/internal/bundler_tests/bundler_importstar_test.go */
  import_star: {
    name: 'import_star',
    sourcePath: './bundler_importstar_test.go',
  },
})
/**
 * The key of the cases constant. {@link cases}
 * @typedef {keyof cases} TestCaseName
 */

/**
 * An object with properties describing the test case.
 * @typedef {cases[keyof cases]} TestCase
 */

/**
 * Attempts to read the .go source file based on the provided test case name. {@link cases}
 * @param {TestCaseName} testCaseName - The name of the current test case.
 * @returns {string} The contents of the .go test source file.
 *
 * ## Panics
 * Performs {@link process.exit} with helpful text error if cannot find .go source file based on test case name {@link cases}
 */
function readTestCaseSource(testCaseName) {
  const sourcePath = path.resolve(
    import.meta.dirname,
    cases[testCaseName].sourcePath,
  )
  try {
    return fs.readFileSync(sourcePath).toString()
  } catch (err) {
    console.log(
      chalk.red(
        `Could not read ${cases[testCaseName].sourcePath} source test file at path: ${sourcePath}`,
      ),
    ),
      process.exit(1)
  }
}

/** @type {TestCaseName} */
const TEST_CASE_NAME = 'default'
const currentCase = cases[TEST_CASE_NAME]
/** The contents of the .go test source file. {@link cases} */
const source = readTestCaseSource(TEST_CASE_NAME)
const ignoredTestName = [
  'ts',
  'txt',
  'json',
  'jsx',
  'tsx',
  'no_bundle',
  'mangle',
  'minify',
  'minified',
  'comments',
  'fs',
  'alias',
  'node',
  'decorator',
  'iife',
  'abs_path',
  'inject',
  'metafile',
  'output_extension',
  'top_level_return_forbidden',
]
const parser = new Parser()
parser.setLanguage(Go)

const tree = parser.parse(source)

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
`

/**
 * @param {import("tree-sitter").SyntaxNode} root
 * @returns {Record<string, Parser.SyntaxNode>}
 * */
function getTopLevelBinding(root) {
  /** @type {Record<string, Parser.SyntaxNode>} */
  const binding = {}
  root.namedChildren.forEach((child) => {
    if (child.type === 'var_declaration') {
      const var_spec = child.namedChildren[0]
      const name = var_spec.namedChild(0)?.text
      const decl = var_spec.namedChild(1)
      if (!name || !decl) {
        return
      }
      binding[name] = decl
    }
  })
  return binding
}

let topLevelBindingMap = getTopLevelBinding(tree.rootNode)
let query = new Parser.Query(parser.getLanguage(), queryString)

function isDirEmptySync(dir) {
  let list = fs.readdirSync(dir)
  return list.length === 0
}

for (let i = 0, len = tree.rootNode.namedChildren.length; i < len; i++) {
  let child = tree.rootNode.namedChild(i)
  if (child.type == 'function_declaration') {
    let testCaseName = child.namedChild(0).text
    testCaseName = testCaseName.slice(4)
    testCaseName = changeCase.snakeCase(testCaseName)

    console.log(testCaseName)
    // Skip some test cases by ignoredTestName
    if (ignoredTestName.some((name) => testCaseName.includes(name))) {
      continue
    }
    let testDir = path.resolve(
      __dirname,
      `../crates/rolldown/tests/esbuild/${currentCase.name}`,
      testCaseName,
    )
    let ignoredTestDir = path.resolve(
      __dirname,
      `../crates/rolldown/tests/esbuild/${currentCase.name}`,
      `.${testCaseName}`,
    )

    // Cause if you withdraw directory in git system, git will cleanup dir but leave the directory alone
    if (
      (fs.existsSync(testDir) && !isDirEmptySync(testDir)) ||
      (fs.existsSync(ignoredTestDir) && !isDirEmptySync(ignoredTestDir))
    ) {
      continue
    } else {
      fs.ensureDirSync(testDir)
    }
    let bundle_field_list = query.captures(child).filter((item) => {
      return item.name === 'element_list'
    })
    let jsConfig = Object.create(null)
    bundle_field_list.forEach((cap) => {
      processKeyElement(cap.node, jsConfig, topLevelBindingMap)
    })

    const fileList = jsConfig['files']
    // Skip jsx/ts/tsx files test case
    if (
      fileList.some(
        (file) =>
          file.name.endsWith('ts') ||
          file.name.endsWith('tsx') ||
          file.name.endsWith('jsx'),
      )
    ) {
      continue
    }
    let prefix = calculatePrefix(fileList.map((item) => item.name))
    fileList.forEach((file) => {
      let normalizedName = file.name.slice(prefix.length)
      if (path.isAbsolute(normalizedName)) {
        normalizedName = normalizedName.slice(1)
      }
      const absFile = path.resolve(testDir, normalizedName)
      const dirName = path.dirname(absFile)
      fs.ensureDirSync(dirName)
      fs.writeFileSync(absFile, file.content)
    })

    // entry
    const config = { input: {} }
    const entryPaths = jsConfig['entryPaths'] ?? []
    if (!entryPaths.length) {
      console.error(chalk.red(`No entryPaths found`))
    }
    let input = entryPaths.map((p) => {
      let normalizedName = p.slice(prefix.length)
      if (path.isAbsolute(normalizedName)) {
        normalizedName = normalizedName.slice(1)
      }
      return {
        name: normalizedName.split('/').join('_').split('.').join('_'),
        import: normalizedName,
      }
    })
    config.input.input = input
    const configFilePath = path.resolve(testDir, 'test.config.json')
    fs.writeFileSync(configFilePath, JSON.stringify(config, null, 2))
    // TODO: options

    let log = jsConfig['expectedCompileLog']
    if (log) {
      const configFilePath = path.resolve(testDir, 'compile-log.text')
      fs.writeFileSync(configFilePath, log)
    }
  }
}

function calculatePrefix(stringList) {
  if (stringList.length < 2) {
    return ''
  }
  let res = ''
  while (true) {
    if (stringList[0][res.length]) {
      res += stringList[0][res.length]
    } else {
      break
    }
    for (let i = 0; i < stringList.length; i++) {
      if (!stringList[i].startsWith(res)) {
        return res.slice(0, res.length - 1)
      }
    }
  }
  return res
}

/**
 * @param {import('tree-sitter').SyntaxNode} node
 * @param {{[x: string]: import('tree-sitter').SyntaxNode} } binding
 */
function processFiles(node, binding) {
  if (node.firstChild.type === 'identifier') {
    let name = node.firstChild.text
    if (binding[name]) {
      node = binding[name]
    }
  }
  let fileList = []
  let compositeLiteral = node.namedChild(0)
  let body = compositeLiteral.namedChild(1)
  try {
    body.namedChildren.forEach((child) => {
      if (child.type !== 'keyed_element') {
        return
      }
      let name = child.namedChild(0)?.text.slice(1, -1)
      let content = child.namedChild(1).text.slice(1, -1).trim()
      content = dedent.default(content)
      fileList.push({
        name,
        content,
      })
    })
    return fileList
  } catch (err) {
    console.error(`Error occurred when processFiles: ${chalk.red(err)}`)
    return []
  }
}

/**
 * @param {import('tree-sitter').SyntaxNode} node
 * @param {[x: string]: import('tree-sitter').SyntaxNode} binding
 */
function processEntryPath(node, binding) {
  if (node.firstChild.type === 'identifier') {
    let name = node.firstChild.text
    if (binding[name]) {
      node = binding[name]
    }
  }
  let entryList = []
  let compositeLiteral = node.namedChild(0)
  let body = compositeLiteral.namedChild(1)
  try {
    body.namedChildren.forEach((child) => {
      let entry = child.namedChild(0).text.slice(1, -1)
      entryList.push(entry)
    })

    return entryList
  } catch (err) {
    console.error(`Error occurred when processEntryPath: ${chalk.red(err)}`)
    return []
  }
}

// TODO only preserve mode ModeBundle test case
/**
 * @param {import('tree-sitter').SyntaxNode} node
 */
function processOptions(node) {}

/**
 * @param {import('tree-sitter').SyntaxNode} node
 * @param {*} config
 * @param {{[x: string]: import('tree-sitter').SyntaxNode} } binding
 *
 */
function processKeyElement(node, config, binding) {
  let keyValue = node.namedChild(0).text
  switch (keyValue) {
    case 'files':
      config['files'] = processFiles(node.namedChild(1), binding)
      break
    case 'entryPaths':
      config['entryPaths'] = processEntryPath(node.namedChild(1), binding)
      break
    case 'options':
      config['options'] = processOptions(node.namedChild(1))
      break
    case 'expectedCompileLog':
      config['expectedCompileLog'] = node.namedChild(1).text.slice(1, -1)
      break
    default:
      console.log(chalk.yellow(`unknown filed ${keyValue}`))
      break
  }
}
