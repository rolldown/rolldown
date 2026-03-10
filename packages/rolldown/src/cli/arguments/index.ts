import cac from 'cac';
import { getCliSchemaInfo } from '../../utils/validator';
import { logger } from '../logger';
import { alias, type CliOptions } from './alias';
import { normalizeCliOptions, type NormalizedCliOptions } from './normalize';
import { camelCaseToKebabCase } from './utils';

const schemaInfo = getCliSchemaInfo();

// Build the options export for help message
export const options: {
  [k: string]: {
    type: 'boolean' | 'string';
    short?: string;
    hint?: string;
    description: string;
  };
} = Object.fromEntries(
  Object.entries(schemaInfo)
    .filter(([_key, info]) => info.type !== 'never')
    .map(([key, info]) => {
      const config = alias[key as keyof typeof alias];

      let description = info?.description ?? config?.description ?? '';
      if (config?.reverse) {
        if (description.startsWith('enable')) {
          description = description.replace('enable', 'disable');
        } else if (!description.startsWith('Avoid')) {
          description = `disable ${description}`;
        }
      }

      const result: {
        type: 'boolean' | 'string';
        short?: string;
        hint?: string;
        description: string;
      } = {
        type: info.type === 'boolean' ? 'boolean' : 'string',
        description,
      };
      if (config?.abbreviation) {
        result.short = config.abbreviation;
      }
      if (config?.hint) {
        result.hint = config.hint;
      }

      const kebabKey = camelCaseToKebabCase(key);
      const optionKey = config?.reverse ? `no-${kebabKey}` : kebabKey;
      return [optionKey, result];
    }),
);

const knownKeys = new Set(Object.keys(schemaInfo));
for (const key of Object.keys(schemaInfo)) {
  const dotIdx = key.indexOf('.');
  if (dotIdx > 0) {
    knownKeys.add(key.substring(0, dotIdx));
  }
}

const shortAliases = new Set<string>();
for (const config of Object.values(alias)) {
  if (config?.abbreviation) {
    shortAliases.add(config.abbreviation);
  }
}

export function parseCliArguments(): NormalizedCliOptions & {
  rawArgs: Record<string, any>;
} {
  const cli = cac('rolldown');

  // Register all options with cac
  for (const [key, info] of Object.entries(schemaInfo)) {
    if (info.type === 'never') continue;
    const config = alias[key as keyof typeof alias];

    let rawName = '';
    if (config?.abbreviation) rawName += `-${config.abbreviation}, `;

    if (config?.reverse) {
      rawName += `--no-${key}`;
    } else {
      rawName += `--${key}`;
    }

    if (info.type !== 'boolean' && !config?.reverse) {
      if (config?.requireValue) {
        rawName += ` <${config?.hint ?? key}>`;
      } else {
        rawName += ` [${config?.hint ?? key}]`;
      }
    }

    cli.option(rawName, info.description ?? config?.description ?? '');
  }

  let parsedInput: string[] = [];
  let parsedOptions: Record<string, any> = {};

  const cmd = cli.command('[...input]', '');
  cmd.allowUnknownOptions();

  // Disable applying default values.
  //
  // For options with prefix `--no-*`, cac sets `true` by default.
  // For example, `--no-preserve-entry-signatures` will set `preserveEntrySignatures` to `true` by default. However, we want it to be `undefined` by default.
  //
  // Here we disable cac's default behavior and let the bundler's internal default value handling logic handle it.
  cmd.ignoreOptionDefaultValue();

  cmd.action((input: string[], opts: Record<string, any>) => {
    parsedInput = input;
    parsedOptions = opts;
  });

  try {
    cli.parse(process.argv, { run: true });
  } catch (err: any) {
    if (err?.name === 'CACError') {
      const match = err.message.match(/option `(.+?)` value is missing/);
      if (match) {
        const optName = match[1].replace(/ [<[].*/, '').replace(/^-\w, /, '');
        logger.error(`Option \`${optName}\` requires a value but none was provided.`);
      } else {
        logger.error(err.message);
      }
      process.exit(1);
    }
    throw err;
  }

  // Post-processing

  // CAC collects arguments behind `--` in a separate `--` key.
  // This is unknown to the bundler.
  delete parsedOptions['--'];

  // Remove short-alias keys (cac/mri duplicates them alongside the full name)
  for (const short of shortAliases) {
    delete parsedOptions[short];
  }

  // Prototype pollution guard
  for (const key of Object.keys(parsedOptions)) {
    if (
      key === '__proto__' ||
      key === 'constructor' ||
      key === 'prototype' ||
      key.startsWith('__proto__.') ||
      key.startsWith('constructor.') ||
      key.startsWith('prototype.')
    ) {
      delete parsedOptions[key];
    }
  }

  // Unknown option detection + warning
  const unknownKeys = Object.keys(parsedOptions).filter((k) => !knownKeys.has(k));

  if (unknownKeys.length > 0) {
    unknownKeys.sort();
    const single = unknownKeys.length === 1;
    logger.warn(
      `Option \`${unknownKeys.join(',')}\` ${single ? 'is' : 'are'} unrecognized. ` +
        `We will ignore ${single ? 'this' : 'those'} option${single ? '' : 's'}.`,
    );
  }

  // rawArgs assembly — snapshot before removing unknown keys
  const rawArgs: Record<string, any> = { ...parsedOptions };

  // Remove unknown keys from parsedOptions before type coercion
  for (const key of unknownKeys) {
    delete parsedOptions[key];
  }

  // Type coercion — duplicate filtering + array wrapping
  for (const [key, value] of Object.entries(parsedOptions)) {
    const type = schemaInfo[key]?.type;
    if (Array.isArray(value)) {
      if (type !== 'array' && type !== 'object') {
        parsedOptions[key] = value[value.length - 1];
      }
    } else if (type === 'array' && typeof value === 'string') {
      parsedOptions[key] = [value];
    }
  }

  // Object option parsing — parse "key:val,key:val" strings (Rollup-compatible)
  // Also supports deprecated "key=val,key=val" syntax with a warning
  for (const [schemaKey, info] of Object.entries(schemaInfo)) {
    if (info.type !== 'object') continue;

    const parts = schemaKey.split('.');
    let parent: any = parsedOptions;
    for (let i = 0; i < parts.length - 1; i++) {
      parent = parent?.[parts[i]];
    }
    const leafKey = parts[parts.length - 1];
    const value = parent?.[leafKey];
    if (value === undefined) continue;

    const values = Array.isArray(value) ? value : [value];
    if (typeof values[0] !== 'string') continue;

    let usedDeprecatedSyntax = false;
    const result: Record<string, string> = {};
    for (const v of values) {
      for (const pair of String(v).split(',')) {
        // Prefer `:` only if it appears before `=` (or `=` doesn't exist)
        // This ensures `key=value:with:colon` (deprecated) is parsed correctly
        const colonIdx = pair.indexOf(':');
        const eqIdx = pair.indexOf('=');
        let k: string;
        let val: string;
        if (colonIdx > 0 && (eqIdx === -1 || colonIdx < eqIdx)) {
          k = pair.slice(0, colonIdx);
          val = pair.slice(colonIdx + 1);
        } else if (eqIdx > 0) {
          k = pair.slice(0, eqIdx);
          val = pair.slice(eqIdx + 1);
          usedDeprecatedSyntax = true;
        } else {
          continue;
        }
        result[k] = val;
      }
    }
    if (usedDeprecatedSyntax) {
      const optionName = camelCaseToKebabCase(schemaKey);
      logger.warn(
        `Using \`key=value\` syntax for \`--${optionName}\` is deprecated. Use \`key:value\` instead.`,
      );
    }
    parent[leafKey] = result;
  }

  const normalizedOptions = normalizeCliOptions(parsedOptions as CliOptions, parsedInput);
  return { ...normalizedOptions, rawArgs };
}
