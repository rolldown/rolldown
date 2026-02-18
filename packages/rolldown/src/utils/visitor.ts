// @ts-ignore
import { walkProgram } from 'oxc-parser/src-js/generated/visit/walk.js';
import {
  addVisitorToCompiled,
  createCompiledVisitor,
  finalizeCompiledVisitor,
  // @ts-ignore
} from 'oxc-parser/src-js/visit/visitor.js';
import type { VisitorObject } from 'oxc-parser';
import type { Program } from '@oxc-project/types';

export type { VisitorObject } from 'oxc-parser';

// This is a re-implementation of oxc-parser's Visitor that uses static ESM imports
// instead of `createRequire`, so it works correctly after bundling.
/**
 * Visitor class for traversing AST.
 *
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
