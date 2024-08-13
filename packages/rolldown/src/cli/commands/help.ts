import { logger } from '../utils'
import {
  version,
  description,
} from '../../../package.json' assert { type: 'json' }
import { CLI_OPTIONS } from '../options'
import { bold, cyan, gray, underline } from '../colors'

const HELP_TEMPLATE = `${gray(`${description} (rolldown v${version})`)}

${bold(underline('USAGE'))} ${cyan('rolldown [OPTIONS]')}

${bold(underline('OPTIONS'))}
__OPTIONS__
`

export function showHelp() {
  logger.log(
    HELP_TEMPLATE.replace(
      '__OPTIONS__',
      Object.entries(CLI_OPTIONS)
        .map(([option, { short, hint, description }]) => {
          let optionStr = '  '
          if (short) {
            optionStr += `-${short}, `
          }
          optionStr += `--${option} `
          if (hint) {
            optionStr += `<${hint}> `
          }
          return cyan(optionStr.padEnd(30)) + description
        })
        .join('\n'),
    ),
  )
}
