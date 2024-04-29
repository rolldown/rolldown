/**
 * Custom usage for citty.
 * This module is based on the following URL from citty
 * https://github.com/unjs/citty/blob/696e5ee8eee9e3b6b10666d82db4899ebf4d5074/src/usage.ts
 */

import { colors } from 'consola/utils'
import {
  CommandDef,
  ArgsDef,
  ArgDef,
  Arg,
  Resolvable,
  BooleanArgDef,
  StringArgDef,
} from 'citty'
import { logger } from './utils.js'
import { brandColor } from './colors.js'
import { arraify } from '../utils/index.js'

export async function showUsage<T extends ArgsDef = ArgsDef>(
  cmd: CommandDef<T>,
  parent?: CommandDef<T>,
) {
  try {
    logger.log((await renderUsage(cmd, parent)) + '\n')
  } catch (error) {
    logger.error(error)
  }
}

export async function renderUsage<T extends ArgsDef = ArgsDef>(
  cmd: CommandDef<T>,
  _parent?: CommandDef<T>,
) {
  const cmdMeta = await resolveValue(cmd.meta || {})
  const cmdArgs = resolveArgs(await resolveValue(cmd.args || {}))

  const argLines: string[][] = []
  const usageLine = []

  for (const arg of cmdArgs) {
    const isRequired = arg.required === true && arg.default === undefined

    const argStr =
      (isBooleanArgDef(arg) && arg.default === true
        ? [
            ...(arg.alias || []).map((a) => `--no-${a}`),
            `--no-${arg.name}`,
          ].join(', ')
        : [...(arg.alias || []).map((a) => `-${a}`), `--${arg.name}`].join(
            ', ',
          )) +
      (isStringArgDef(arg) && (arg.valueHint || arg.default)
        ? `=${arg.valueHint ? `<${arg.valueHint}>` : `"${arg.default || ''}"`}`
        : '')

    argLines.push([
      colors.cyan(argStr + (isRequired ? ' (required)' : '')),
      arg.description || '',
    ])

    if (isRequired) {
      usageLine.push(argStr)
    }
  }

  const usageLines: (string | undefined)[] = []

  const commandName = cmdMeta.name || process.argv[1]
  usageLines.push(
    colors.bold(
      brandColor(
        `${
          commandName + (cmdMeta.version ? ` ${cmdMeta.version}` : '')
        } - ${cmdMeta.description}`,
      ),
    ),
    '',
  )

  const hasOptions = argLines.length > 0
  usageLines.push(
    `${colors.underline(colors.bold('USAGE:'))} ${colors.cyan(
      `${commandName}${hasOptions ? ' [OPTIONS]' : ''} ${usageLine.join(' ')}`,
    )}`,
    '',
  )

  if (argLines.length > 0) {
    usageLines.push(colors.underline(colors.bold('OPTIONS:')), '')
    usageLines.push(formatLineColumns(argLines, '  '))
    usageLines.push('')
  }

  return usageLines.filter((l) => typeof l === 'string').join('\n')
}

function resolveValue<T>(input: Resolvable<T>): T | Promise<T> {
  return typeof input === 'function' ? (input as any)() : input
}

function resolveArgs(argsDef: ArgsDef): Arg[] {
  const args: Arg[] = []
  for (const [name, argDef] of Object.entries(argsDef || {})) {
    args.push({
      ...argDef,
      name,
      alias: arraify((argDef as any).alias),
    })
  }
  return args
}

function isStringArgDef(arg: ArgDef): arg is StringArgDef {
  return arg.type === 'string'
}

function isBooleanArgDef(arg: ArgDef): arg is BooleanArgDef {
  return arg.type === 'boolean'
}

function formatLineColumns(lines: string[][], linePrefix = '') {
  const maxLength: number[] = []
  for (const line of lines) {
    for (const [i, element] of line.entries()) {
      maxLength[i] = Math.max(maxLength[i] || 0, element.length)
    }
  }
  return lines
    .map((l) =>
      l
        .map(
          (c, i) =>
            linePrefix + c[i === 0 ? 'padStart' : 'padEnd'](maxLength[i]),
        )
        .join('  '),
    )
    .join('\n')
}
