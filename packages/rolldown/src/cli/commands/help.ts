import { logger } from '../utils'
import {
  version,
  description,
} from '../../../package.json' assert { type: 'json' }
import { bold, cyan, gray, underline } from '../colors'
import { options } from '../arguments'
import { alias, OptionConfig } from '../arguments/alias'
import { camelCaseToKebabCase } from '../arguments/utils'

const introduction = `${gray(`${description} (rolldown v${version})`)}

${bold(underline('USAGE'))} ${cyan('rolldown -c <config>')} or ${cyan('rolldown <input> <options>')}`

const examples = [
  {
    title: 'Bundle with a config file `rolldown.config.mjs`',
    command: 'rolldown -c rolldown.config.mjs',
  },
  {
    title: 'Bundle the `src/main.ts` to `dist` with `cjs` format',
    command: 'rolldown src/main.ts -d dist -f cjs',
  },
  {
    title: 'Bundle the `src/main.ts` and handle the `.png` assets to Data URL',
    command: 'rolldown src/main.ts -d dist --moduleTypes .png=dataurl',
  },
  {
    title: 'Bundle the `src/main.tsx` and minify the output with sourcemap',
    command: 'rolldown src/main.tsx -d dist -m -s',
  },
  {
    title: 'Create self-executing IIFE using external jQuery as `$` and `_`',
    command:
      'rolldown src/main.ts -d dist -n bundle -f iife -e jQuery,window._ -g jQuery=$',
  },
]

const notes = [
  'Due to the API limitation, you need to pass `-s` for `.map` sourcemap file as the last argument.',
  'If you are using the configuration, please pass the `-c` as the last argument if you ignore the default configuration file.',
  'CLI options will override the configuration file.',
  'For more information, please visit https://rolldown.rs/.',
]

export function showHelp() {
  logger.log(introduction)
  logger.log('')
  logger.log(`${bold(underline('OPTIONS'))}`)
  logger.log('')
  logger.log(
    Object.entries(options)
      .sort(([a], [b]) => {
        // 1. If one of them has a short option, prioritize it.
        if (options[a].short && !options[b].short) {
          return -1
        }
        if (!options[a].short && options[b].short) {
          return 1
        }
        // 2. If both of them have a short option, sort by the short letter.
        if (options[a].short && options[b].short) {
          return options[a].short.localeCompare(options[b].short)
        }
        // 3. If none of them has a short option, sort by the long option.
        return a.localeCompare(b)
      })
      .map(([option, { type, short, hint, description }]) => {
        let optionStr = '  '
        const config = ((Object.getOwnPropertyDescriptor(alias, option) ?? {})
          .value ?? {}) as OptionConfig
        option = camelCaseToKebabCase(option)
        if (
          typeof config.default === 'boolean' &&
          type === 'boolean' &&
          config.default
        ) {
          optionStr += `--no-${option}`
          description = `Do not ${description}`
        } else {
          optionStr += `--${option} `
        }
        if (short) {
          optionStr += `-${short}, `
        }
        if (type === 'string') {
          optionStr += `<${hint ?? option}>`
        }
        if (description && description.length > 0) {
          description = description[0].toUpperCase() + description.slice(1)
        }
        return (
          cyan(optionStr.padEnd(30)) +
          description +
          (description && description?.endsWith('.') ? '' : '.')
        )
      })
      .join('\n'),
  )
  logger.log('')
  logger.log(`${bold(underline('EXAMPLES'))}`)
  logger.log('')
  examples.forEach(({ title, command }, ord) => {
    logger.log(`  ${ord + 1}. ${title}:`)
    logger.log(`    ${cyan(command)}`)
    logger.log('')
  })
  logger.log(`${bold(underline('NOTES'))}`)
  logger.log('')
  notes.forEach((note) => {
    logger.log(`  * ${gray(note)}`)
  })
}
