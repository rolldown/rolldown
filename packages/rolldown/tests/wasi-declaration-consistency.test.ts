import { readFile } from 'node:fs/promises';

import * as ts from 'typescript';
import { expect, test } from 'vitest';

const declarations = {
  native: new URL('../src/binding.d.cts', import.meta.url),
  threaded: new URL('../src/rolldown-binding.wasi.d.cts', import.meta.url),
  threadless: new URL('../src/rolldown-binding.wasip1.d.cts', import.meta.url),
} as const;

const targetNeutralTypes = [
  'MangleOptions',
  'BindingNormalizedOptions',
  'BindingOutputOptions',
  'JsOutputChunk',
] as const;

function getDeclarationMembers(source: string, filename: string, typeName: string): string[] {
  const sourceFile = ts.createSourceFile(
    filename,
    source,
    ts.ScriptTarget.Latest,
    true,
    ts.ScriptKind.TS,
  );
  const declaration = sourceFile.statements.find(
    (statement): statement is ts.InterfaceDeclaration | ts.ClassDeclaration =>
      (ts.isInterfaceDeclaration(statement) || ts.isClassDeclaration(statement)) &&
      statement.name?.text === typeName,
  );
  if (!declaration) {
    throw new Error(`Missing ${typeName} in ${filename}`);
  }

  const printer = ts.createPrinter({ removeComments: true });
  return declaration.members
    .map((member) =>
      printer.printNode(ts.EmitHint.Unspecified, member, sourceFile).replaceAll(/\s+/g, ' ').trim(),
    )
    .sort();
}

test('keeps target-neutral binding types aligned across native and WASI declarations', async () => {
  const sources = Object.fromEntries(
    await Promise.all(
      Object.entries(declarations).map(async ([flavor, path]) => [
        flavor,
        await readFile(path, 'utf8'),
      ]),
    ),
  ) as Record<keyof typeof declarations, string>;

  for (const typeName of targetNeutralTypes) {
    const expected = getDeclarationMembers(sources.native, declarations.native.pathname, typeName);
    for (const flavor of ['threaded', 'threadless'] as const) {
      expect(
        getDeclarationMembers(sources[flavor], declarations[flavor].pathname, typeName),
        `${typeName} differs in the ${flavor} WASI declaration`,
      ).toEqual(expected);
    }
  }
});
