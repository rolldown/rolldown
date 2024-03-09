// ISC License

// Copyright (c) 2021 Alexey Raspopov, Kostiantyn Denysov, Anton Verinov

// Permission to use, copy, modify, and/or distribute this software for any
// purpose with or without fee is hereby granted, provided that the above
// copyright notice and this permission notice appear in all copies.

// THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHOR DISCLAIMS ALL WARRANTIES
// WITH REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF
// MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR
// ANY SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES
// WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN AN
// ACTION OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT OF
// OR IN CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.

// brand gradient colors
const argv = process.argv || [],
  env = process.env

const enabled =
  !('NO_COLOR' in env || argv.includes('--no-color')) &&
  ('FORCE_COLOR' in env ||
    argv.includes('--color') ||
    process.platform === 'win32' ||
    (require != null && require('tty').isatty(1) && env.TERM !== 'dumb') ||
    'CI' in env)

const createFormatter =
  (open, close, replace = open) =>
  (input) => {
    const string = '' + input
    const index = string.indexOf(close, open.length)
    return ~index
      ? open + replaceClose(string, close, replace, index) + close
      : open + string + close
  }

const replaceClose = (string, close, replace, index) => {
  const start = string.substring(0, index) + replace
  const end = string.substring(index + close.length)
  const nextIndex = end.indexOf(close)
  return ~nextIndex
    ? start + replaceClose(end, close, replace, nextIndex)
    : start + end
}

const reset = enabled ? (s) => `\x1b[0m${s}\x1b[0m` : String
const bold = enabled
  ? createFormatter('\x1b[1m', '\x1b[22m', '\x1b[22m\x1b[1m')
  : String
const dim = enabled
  ? createFormatter('\x1b[2m', '\x1b[22m', '\x1b[22m\x1b[2m')
  : String
const italic = enabled ? createFormatter('\x1b[3m', '\x1b[23m') : String
const underline = enabled ? createFormatter('\x1b[4m', '\x1b[24m') : String
const inverse = enabled ? createFormatter('\x1b[7m', '\x1b[27m') : String
const hidden = enabled ? createFormatter('\x1b[8m', '\x1b[28m') : String
const strikethrough = enabled ? createFormatter('\x1b[9m', '\x1b[29m') : String

const debugColor = createFormatter('\x1b[38;2;255;140;0m', '\x1b[39m')

// black
const black = enabled ? createFormatter('\x1b[38;2;0;0;0m', '\x1b[39m') : String
const red = enabled
  ? createFormatter('\x1b[38;2;219;90;107m', '\x1b[39m')
  : String
const green = enabled ? createFormatter('\x1b[32m', '\x1b[39m') : String
const yellow = enabled ? createFormatter('\x1b[33m', '\x1b[39m') : String
const blue = enabled
  ? createFormatter('\x1b[38;2;68;206;246m', '\x1b[39m')
  : String
const magenta = enabled
  ? createFormatter('\x1b[38;2;180;0;100m', '\x1b[39m')
  : String
const purple = enabled
  ? createFormatter('\x1b[38;2;140;67;86m', '\x1b[39m')
  : String
const orange = enabled
  ? createFormatter('\x1b[38;2;255;137;54m', '\x1b[39m')
  : String
const cyan = enabled ? createFormatter('\x1b[36m', '\x1b[39m') : String
const white = enabled ? createFormatter('\x1b[37m', '\x1b[39m') : String
const gray = enabled
  ? createFormatter('\x1b[38;2;128;128;128m', '\x1b[39m')
  : String
const bgBlack = enabled ? createFormatter('\x1b[40m', '\x1b[49m') : String
const bgRed = enabled ? createFormatter('\x1b[41m', '\x1b[49m') : String
const bgGreen = enabled ? createFormatter('\x1b[42m', '\x1b[49m') : String
const bgYellow = enabled ? createFormatter('\x1b[43m', '\x1b[49m') : String
const bgBlue = enabled ? createFormatter('\x1b[44m', '\x1b[49m') : String
const bgMagenta = enabled ? createFormatter('\x1b[45m', '\x1b[49m') : String
const bgCyan = enabled ? createFormatter('\x1b[46m', '\x1b[49m') : String
const bgWhite = enabled ? createFormatter('\x1b[47m', '\x1b[49m') : String

const colors = {
  reset,
  bold,
  dim,
  italic,
  underline,
  inverse,
  hidden,
  strikethrough,
  black,
  red,
  green,
  yellow,
  blue,
  magenta,
  purple,
  orange,
  cyan,
  gray,
  white,
  bgBlack,
  bgRed,
  bgGreen,
  bgYellow,
  bgBlue,
  bgMagenta,
  bgCyan,
  bgWhite,
  debugColor,
}

module.exports = colors
