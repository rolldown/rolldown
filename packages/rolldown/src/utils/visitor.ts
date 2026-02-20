// @ts-ignore
import { walkProgram } from 'oxc-parser/src-js/generated/visit/walk.js';
import {
  addVisitorToCompiled,
  createCompiledVisitor,
  finalizeCompiledVisitor,
  // @ts-ignore
} from 'oxc-parser/src-js/visit/visitor.js';
import type { VisitorObject as OriginalVisitorObject } from 'oxc-parser';
import type { Program } from '@oxc-project/types';

/**
 * Visitor object for traversing AST.
 *
 * @category Utilities
 */
export type VisitorObject = OriginalVisitorObject;

// This is a re-implementation of oxc-parser's Visitor that uses static ESM imports
// instead of `createRequire`, so it works correctly after bundling.
/**
 * Visitor class for traversing AST.
 *
 * @example
 * ```ts
 * import { Visitor } from 'rolldown/utils';
 * import { parseSync } from 'rolldown/utils';
 *
 * const result = parseSync(...);
 * const visitor = new Visitor({
 *   VariableDeclaration(path) {
 *     // Do something with the variable declaration
 *   },
 *   "VariableDeclaration:exit"(path) {
 *     // Do something after visiting the variable declaration
 *   }
 * });
 * visitor.visit(result.program);
 * ```
 *
 * @category Utilities
 * @experimental
 */
export class Visitor {
  #compiledVisitor: unknown[] | null = null;

  constructor(visitor: VisitorObject) {
    const compiledVisitor = createCompiledVisitor();
    addVisitorToCompiled(visitor);
    const needsVisit = finalizeCompiledVisitor();
    if (needsVisit) this.#compiledVisitor = compiledVisitor;
  }

  visit(program: Program): void {
    const compiledVisitor = this.#compiledVisitor;
    if (compiledVisitor !== null) walkProgram(program, compiledVisitor);
  }
}
