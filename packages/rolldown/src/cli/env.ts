import tty from 'node:tty'

/**
 * The following flags are based on conosola
 * https://github.com/unjs/consola/blob/24c98ceb90c269a170fd116134d91803a89f2c9d/src/utils/color.ts#L7-L26
 */
const { env, argv, platform } = process
const isDisabled = 'NO_COLOR' in env || argv.includes('--no-color')
const isForced = 'FORCE_COLOR' in env || argv.includes('--color')
const isWindows = platform === 'win32'
const isDumbTerminal = env.TERM === 'dumb'
const isCompatibleTerminal =
  tty && tty.isatty && tty.isatty(1) && env.TERM && !isDumbTerminal
const isCI =
  'CI' in env &&
  ('GITHUB_ACTIONS' in env || 'GITLAB_CI' in env || 'CIRCLECI' in env)

/**
 * Whether the terminal supports color
 */
export const isColorSupported =
  !isDisabled &&
  (isForced || (isWindows && !isDumbTerminal) || isCompatibleTerminal || isCI)

/**
 * The color depth of the terminal
 */
export const colorDepth = tty.WriteStream.prototype.getColorDepth()
