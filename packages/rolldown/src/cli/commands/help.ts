import { logger } from '../utils'
import {
  version,
  description,
} from '../../../package.json' assert { type: 'json' }
import { bold, cyan, gray, underline } from '../colors'
import { options } from '../arguments'

const template = `${gray(`${description} (rolldown v${version})`)}

${bold(underline('USAGE'))} ${cyan('rolldown -c <config>')} or ${cyan('rolldown <input> <options>')}

${bold(underline('OPTIONS'))}
`

export function showHelp() {
  logger.log(
    template +
    Object.entries(options)
      .map(([option, { type, short, hint, description }]) => {
        let optionStr = '  '
        if (short) {
          optionStr += `-${short}, `
        }
        optionStr += `--${option} `
        if (type === 'string') {
          optionStr += `<${hint ?? option}>`
        }
        return cyan(optionStr.padEnd(30)) + description
      })
      .join('\n'),
  )
}
