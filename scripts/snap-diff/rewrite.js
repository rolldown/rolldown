import * as acorn from 'acorn'
import * as gen from 'escodegen'
import * as walk from 'acorn-walk'

/**
 * @param {string} code
 *
 */
export function rewriteRolldown(code) {
  let ast = acorn.parse(code, {
    ecmaVersion: 'latest',
    sourceType: 'module',
  })
  walk.simple(ast, {
    ImportDeclaration(node) {
      let sourceList = ['assert', 'node:assert']
      if (sourceList.includes(node.source.value)) {
        node.type = 'EmptyStatement'
      }
    },
    CallExpression(node) {
      let callee = node.callee
      // rewrite assert.strictEqual(test, 1)
      // rewrite assert.equal(test, 1)
      let assertProperties = ['equal', 'strictEqual']
      if (
        callee.type === 'MemberExpression' &&
        callee.object?.name === 'assert' &&
        assertProperties.includes(callee.property?.name)
      ) {
        let args = node.arguments
        if (args.length === 2) {
          callee.object.name = 'console'
          callee.property.name = 'log'
          args.splice(1, 1)
        }
      }
    },
  })
  let generated = gen.generate(ast, {})
  return generated
    .split('\n')
    .filter((line) => {
      return line !== ';'
    })
    .join('\n')
}

/*
 * @param {string} code
 */
export function rewriteEsbuild(code) {
  let ast = acorn.parse(code, {
    ecmaVersion: 'latest',
    sourceType: 'module',
  })
  return gen.generate(ast)
}
