import type { Program } from '@oxc-project/types';
import type { ParseResult, ParserOptions } from './binding.cjs';
import { locate } from './log/locate-character';
import { augmentCodeLocation, error, logParseError } from './log/logs';
import { getCodeFrame } from './utils/code-frame';
import { parse, parseSync } from './utils/parse';

function wrap(
  result: ParseResult,
  filename: string | undefined,
  sourceText: string,
) {
  if (result.errors.length > 0) {
    return normalizeParseError(filename, sourceText, result.errors);
  }
  return result.program;
}

function normalizeParseError(
  filename: string | undefined,
  sourceText: string,
  errors: ParseResult['errors'],
) {
  let message = `Parse failed with ${errors.length} error${
    errors.length < 2 ? '' : 's'
  }:\n`;
  // Get pos from the first error's first label if available
  const pos = errors[0]?.labels?.[0]?.start;
  for (let i = 0; i < errors.length; i++) {
    if (i >= 5) {
      message += '\n...';
      break;
    }
    const e = errors[i];
    message += e.message +
      '\n' +
      e.labels
        .map((label: any) => {
          const location = locate(sourceText, label.start, { offsetLine: 1 });
          if (!location) {
            return;
          }
          return getCodeFrame(sourceText, location.line, location.column);
        })
        .filter(Boolean)
        .join('\n');
  }
  const log = logParseError(message, filename, pos);
  if (pos !== undefined && filename) {
    augmentCodeLocation(log, pos, sourceText, filename);
  }
  return error(log);
}

const defaultParserOptions: ParserOptions = {
  lang: 'js',
  preserveParens: false,
};

// The api compat to rollup `parseAst` and `parseAstAsync`.

export function parseAst(
  sourceText: string,
  options?: ParserOptions | null,
  filename?: string,
): Program {
  let ast = parseSync(filename ?? 'file.js', sourceText, {
    ...defaultParserOptions,
    ...options,
  });
  return wrap(
    ast,
    filename,
    sourceText,
  );
}

export async function parseAstAsync(
  sourceText: string,
  options?: ParserOptions | null,
  filename?: string,
): Promise<Program> {
  return wrap(
    await parse(filename ?? 'file.js', sourceText, {
      ...defaultParserOptions,
      ...options,
    }),
    filename,
    sourceText,
  );
}

export type { ParseResult, ParserOptions };
