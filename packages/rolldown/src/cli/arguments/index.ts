import { parseArgs } from 'node:util';
import { getCliSchemaInfo } from '../../utils/validator';
import { logger } from '../logger';
import { alias, type OptionConfig } from './alias';
import { normalizeCliOptions, type NormalizedCliOptions } from './normalize';
import { camelCaseToKebabCase, kebabCaseToCamelCase } from './utils';

const schemaInfo = getCliSchemaInfo();

export const options: {
  [k: string]: {
    type: 'boolean' | 'string';
    multiple: boolean;
    short?: string;
    default?: boolean | string | string[];
    hint?: string;
    description: string;
    environment?: string | string[];
  };
} = Object.fromEntries(
  Object.entries(schemaInfo).filter(([_key, info]) => info.type !== 'never')
    .map(([key, info]) => {
      const config = Object.getOwnPropertyDescriptor(alias, key)?.value as
        | OptionConfig
        | undefined;

      const type = info.type;

      const result = {
        type: type === 'boolean' ? 'boolean' : 'string',
        // We only support comma separated mode right now.
        // multiple: type === 'object' || type === 'array',
        description: info?.description ?? config?.description ?? '',
        hint: config?.hint,
      } as {
        type: 'boolean' | 'string';
        multiple: boolean;
        short?: string;
        default?: boolean | string | string[];
        hint?: string;
        description: string;
        environment?: string | string[];
      };
      if (config && config?.abbreviation) {
        result.short = config?.abbreviation;
      }
      if (config && config.reverse) {
        if (result.description.startsWith('enable')) {
          result.description = result.description.replace('enable', 'disable');
        } else if (!result.description.startsWith('Avoid')) {
          result.description = `disable ${result.description}`;
        }
      }
      key = camelCaseToKebabCase(key);
      // add 'no-' prefix for need reverse options
      return [config?.reverse ? `no-${key}` : key, result];
    }),
);

export function parseCliArguments(): NormalizedCliOptions & {
  rawArgs: Record<string, any>;
} {
  const { values, tokens, positionals } = parseArgs({
    options,
    tokens: true,
    allowPositionals: true,
    // We can't use `strict` mode because we should handle the default config file name.
    strict: false,
  });

  // Handle nested object options like --transform.define.KEY VALUE
  // When parseArgs encounters unknown options, it treats them as boolean flags
  // and the value becomes a positional argument. We need to detect this pattern
  // and properly associate values with their options.
  
  // Build a map from token index to positional array index
  const tokenIndexToPositionalIndex = new Map<number, number>();
  let positionalArrayIndex = 0;
  for (const token of tokens) {
    if (token.kind === 'positional') {
      tokenIndexToPositionalIndex.set(token.index, positionalArrayIndex);
      positionalArrayIndex++;
    }
  }
  
  const consumedPositionalIndices = new Set<number>();
  const processedNestedOptions = new Set<string>();
  
  for (let i = 0; i < tokens.length; i++) {
    const token = tokens[i];
    if (token.kind === 'option') {
      const parts = token.name.split('.');
      // Check if this looks like a nested object option (at least 3 parts)
      if (parts.length >= 3) {
        // Check if the parent path corresponds to an object-type option
        const parentPath = parts.slice(0, -1).join('.');
        const parentPathCamel = kebabCaseToCamelCase(parentPath);
        const parentInfo = schemaInfo[parentPathCamel];
        
        if (parentInfo && parentInfo.type === 'object') {
          // This is a nested object option, check if next token is a positional
          const nextToken = tokens[i + 1];
          if (nextToken && nextToken.kind === 'positional' && 
              typeof nextToken.value === 'string') {
            // Associate this positional with the option
            Object.defineProperty(values, token.name, {
              value: nextToken.value,
              enumerable: true,
              configurable: true,
              writable: true,
            });
            // Mark this positional as consumed by its array index
            const positionalIndex = tokenIndexToPositionalIndex.get(nextToken.index);
            if (positionalIndex !== undefined) {
              consumedPositionalIndices.add(positionalIndex);
            }
            // Mark this option as processed
            processedNestedOptions.add(token.name);
          }
        }
      }
    }
  }

  // Remove consumed positionals
  const filteredPositionals = positionals.filter((_, index) => 
    !consumedPositionalIndices.has(index)
  );

  let invalid_options = tokens
    .filter((token) => token.kind === 'option')
    .map((option) => {
      // Skip nested object options that were already processed
      if (processedNestedOptions.has(option.name)) {
        return undefined;
      }
      
      let negative = false;
      if (option.name.startsWith('no-')) {
        // stripe `no-` prefix
        const name = kebabCaseToCamelCase(option.name.substring(3));
        if (name in schemaInfo) {
          // Remove the `no-` in values
          delete values[option.name];
          option.name = name;
          negative = true;
        }
      }
      delete values[option.name]; // Strip the kebab-case options.
      option.name = kebabCaseToCamelCase(option.name);
      let originalInfo = schemaInfo[option.name];
      if (!originalInfo) {
        // Return the summary of invalid option.
        return { name: option.name, value: option.value };
      }
      let type = originalInfo.type;
      if (type === 'string' && typeof option.value !== 'string') {
        let opt = option as { name: string };
        // We should use the default value.
        let defaultValue = Object.getOwnPropertyDescriptor(alias, opt.name)
          ?.value as OptionConfig;
        Object.defineProperty(values, opt.name, {
          value: defaultValue.default ?? '',
          enumerable: true,
          configurable: true,
          writable: true,
        });
      } else if (type === 'object' && typeof option.value === 'string') {
        // Support multiple comma-separated key=value pairs
        const pairs = option.value.split(',').map((x) => x.split('='));
        if (!values[option.name]) {
          Object.defineProperty(values, option.name, {
            value: {},
            enumerable: true,
            configurable: true,
            writable: true,
          });
        }
        for (const [key, value] of pairs) {
          if (key && value) {
            Object.defineProperty(values[option.name], key, {
              value,
              enumerable: true,
              configurable: true,
              writable: true,
            });
          }
        }
      } else if (type === 'array' && typeof option.value === 'string') {
        if (!values[option.name]) {
          Object.defineProperty(values, option.name, {
            value: [],
            enumerable: true,
            configurable: true,
            writable: true,
          });
        }
        (values[option.name] as string[]).push(option.value);
      } else if (type === 'boolean') {
        Object.defineProperty(values, option.name, {
          value: !negative,
          enumerable: true,
          configurable: true,
          writable: true,
        });
      } else if (type === 'union') {
        // We should use the default value.
        let defaultValue = Object.getOwnPropertyDescriptor(alias, option.name)
          ?.value as OptionConfig;
        Object.defineProperty(values, option.name, {
          value: option.value ?? defaultValue?.default ?? '',
          enumerable: true,
          configurable: true,
          writable: true,
        });
      } else {
        Object.defineProperty(values, option.name, {
          value: option.value ?? '',
          enumerable: true,
          configurable: true,
          writable: true,
        });
      }
    }).filter((item) => {
      return item !== undefined;
    });

  invalid_options.sort((a, b) => {
    return a.name.localeCompare(b.name);
  });

  if (invalid_options.length !== 0) {
    let single = invalid_options.length === 1;
    logger.warn(
      `Option \`${invalid_options.map(item => item.name).join(',')}\` ${
        single ? 'is' : 'are'
      } unrecognized. We will ignore ${single ? 'this' : 'those'} option${
        single ? '' : 's'
      }.`,
    );
  }

  let rawArgs = {
    ...values,
    ...invalid_options.reduce((acc, cur) => {
      acc[cur.name] = cur.value;
      return acc;
    }, Object.create(null)),
  };
  const normalizedOptions = normalizeCliOptions(
    values,
    filteredPositionals as string[],
  );
  return { ...normalizedOptions, rawArgs };
}
