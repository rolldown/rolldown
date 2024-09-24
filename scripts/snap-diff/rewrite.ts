import * as acorn from 'acorn'
import * as gen from 'escodegen'
import { traverse, builders as b, Scope } from 'estree-toolkit'

export function rewriteRolldown(code: string) {
  let ast = acorn.parse(code, {
    ecmaVersion: 'latest',
    sourceType: 'module',
  })
  let programScope: Scope | null | undefined
  traverse(ast, {
    $: { scope: true },
    Program(path) {
      programScope = path.scope
    },
    ImportDeclaration(path) {
      let sourceList = ['assert', 'node:assert']
      let node = path.node as acorn.ImportDeclaration
      if (
        node.source.value &&
        sourceList.includes(node.source.value.toString())
      ) {
        path.replaceWith(b.emptyStatement())
      }
    },
    ExpressionStatement(path) {
      let node = path.node as acorn.ExpressionStatement
      // TODO: use configuration to control
      // esbuild don't generate 'use strict' when outputFormat: cjs by default
      // only if there is already a 'use strict'
      if (node.directive === 'use strict') {
        path.replaceWith(b.emptyStatement())
      }
    },
    VariableDeclaration(path) {
      // related to https://esbuild.github.io/faq/#top-level-var
      let node = path.node as acorn.VariableDeclaration
      if (path.scope === programScope) {
        node.kind = 'var'
      }
    },
    CallExpression(path) {
      let node = path.node as acorn.CallExpression
      let callee = node.callee
      // rewrite assert.strictEqual(test, 1)
      // rewrite assert.equal(test, 1)
      // rewrite assert.deepEqual(test, 1)
      let assertProperties = ['equal', 'strictEqual', 'deepEqual']
      if (
        callee.type === 'MemberExpression' &&
        callee.object.type === 'Identifier' &&
        callee.object.name === 'assert' &&
        callee.property.type === 'Identifier' &&
        assertProperties.includes(callee.property.name)
      ) {
        let args = node.arguments
        if (args.length === 2) {
          callee.object.name = 'console'
          callee.property.name = 'log'
          // remove second argument in `console.log`
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

export function rewriteEsbuild(code: string) {
  let ast = acorn.parse(code, {
    ecmaVersion: 'latest',
    sourceType: 'module',
  })
  return gen.generate(ast)
}
