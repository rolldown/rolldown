import * as acorn from 'acorn'
import * as gen from 'escodegen'
import { traverse, builders as b, Scope, NodePath } from 'estree-toolkit'

export function rewriteRolldown(code: string) {
  let ast = acorn.parse(code, {
    ecmaVersion: 'latest',
    sourceType: 'module',
  })
  let programScope: Scope | null | undefined
  let collapsedAssertArgs: acorn.Expression[] = []
  let pathToRemove: NodePath<any, any>[] = []
  let isLastExpressionStatementAssert = false
  traverse(ast, {
    $: { scope: true },
    Program: {
      enter(path) {
        programScope = path.scope
      },
      leave(path) {
        for (let p of pathToRemove) {
          p.remove()
        }
        if (collapsedAssertArgs.length) {
          let consoleLogStmt = b.expressionStatement(
            b.callExpression(
              b.memberExpression(b.identifier('console'), b.identifier('log')),
              collapsedAssertArgs as any,
            ),
          )
          path.node?.body.push(consoleLogStmt)
        }
      },
    },
    ImportDeclaration(path) {
      let sourceList = ['assert', 'node:assert']
      let node = path.node as acorn.ImportDeclaration
      if (
        node.source.value &&
        sourceList.includes(node.source.value.toString())
      ) {
        path.remove()
      }
    },
    ExpressionStatement(path) {
      let node = path.node as acorn.ExpressionStatement
      // TODO: use configuration to control
      // esbuild don't generate 'use strict' when outputFormat: cjs by default
      // only if there is already a 'use strict'
      if (node.directive === 'use strict') {
        pathToRemove.push(path)
        return
      }
      if (path.scope === programScope) {
        if (node.expression.type === 'CallExpression') {
          let arg = extractAssertArgument(node.expression)
          if (arg) {
            isLastExpressionStatementAssert = true
            collapsedAssertArgs.push(arg)
            pathToRemove.push(path)
            return
          }
        }

        if (isLastExpressionStatementAssert && collapsedAssertArgs.length) {
          isLastExpressionStatementAssert = false
          let consoleLogStmt = b.expressionStatement(
            b.callExpression(
              b.memberExpression(b.identifier('console'), b.identifier('log')),
              collapsedAssertArgs as any,
            ),
          )
          path.insertBefore([consoleLogStmt])
        }
      }
    },
    VariableDeclaration(path) {
      // related to https://esbuild.github.io/faq/#top-level-var
      let node = path.node as acorn.VariableDeclaration
      if (path.scope === programScope) {
        node.kind = 'var'
      }
    },
  })
  return gen.generate(ast, {})
}

function extractAssertArgument(
  node: acorn.CallExpression,
): acorn.Expression | undefined {
  let callee = node.callee
  // extract assert.strictEqual(test, 1)
  // extract assert.equal(test, 1)
  // extract assert.deepEqual(test, 1)
  let assertProperties = ['equal', 'strictEqual', 'deepEqual']
  if (
    callee.type === 'MemberExpression' &&
    callee.object.type === 'Identifier' &&
    callee.object.name === 'assert' &&
    callee.property.type === 'Identifier' &&
    assertProperties.includes(callee.property.name)
  ) {
    let args = node.arguments
    return args[0] as acorn.Expression
  }
}

export function rewriteEsbuild(code: string) {
  let ast = acorn.parse(code, {
    ecmaVersion: 'latest',
    sourceType: 'module',
  })
  traverse(ast, {
    CallExpression(path) {
      let node = path.node as acorn.CallExpression
      let callee = node.callee
      if (callee.type === 'Identifier' && callee.name === '__commonJS') {
        callee.name = '__commonJSMin'
        if (node.arguments[0].type === 'ObjectExpression') {
          let obj = node.arguments[0]
          let prop = obj.properties[0] as acorn.Property
          // @ts-ignore
          node.arguments[0] = functionExprToArrowFunction(
            prop.value as acorn.FunctionExpression,
          )
        }
      }
    },
  })
  return gen.generate(ast)
}

function functionExprToArrowFunction(node: acorn.FunctionExpression) {
  let arrowFunction = b.arrowFunctionExpression(
    // @ts-ignore
    node.params,
    node.body,
    node.async,
    node.generator,
  )
  return arrowFunction
}
