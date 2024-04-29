/**
 * This module defines the brand color of the rolldown CLI.
 * That color is to be displayed appropriately for the terminal environment.
 * That color is defined using ansi escape code.
 * See: https://en.wikipedia.org/wiki/ANSI_escape_code
 */

import { isColorSupported, colorDepth } from './env.js'

const ANSI_COLOR_OFFSET = 38
const ESC = '\u001B'

type ColorClose = 39 | 49 // '39' for foreground color, '49' for background color
type ColorFunction = (str: string) => string

const defineAnsi16 =
  (open: number, close: ColorClose, offset = 0): ColorFunction =>
  (str: string) =>
    `${ESC}[${open + offset}m${str}${ESC}[${close}m`

const defineAnsi256 =
  (open: number, close: ColorClose, offset = 0): ColorFunction =>
  (str: string) =>
    `${ESC}[${ANSI_COLOR_OFFSET + offset};5;${open}m${str}${ESC}[${close}m`

const defineAnsi16m =
  (
    red: number,
    green: number,
    blue: number,
    close: ColorClose,
    offset = 0,
  ): ColorFunction =>
  (str: string) =>
    `${ESC}[${ANSI_COLOR_OFFSET + offset};2;${red};${green};${blue}m${str}${ESC}[${close}m`

const BRAND_COLOR_TABLE: Record<number, ColorFunction> = {
  // '33' is ansi yellow color
  4: defineAnsi16(33, 39),
  // '178' is rolldown orange color for ansi 256
  8: defineAnsi256(178, 39),
  // 'rgb(227, 151, 9)' is rolldown orange color for ansi 16m (true color), that color take from vitepress `--vp-c-brand-1` css variable
  24: defineAnsi16m(227, 151, 9, 39),
}

/**
 * get the brand color for rolldown cli
 * @description
 * This function supports terminals that color and those that do not
 * @param str a target string
 * @returns a string with brand color
 */
export function brandColor(str: string) {
  if (!isColorSupported) {
    return str
  }
  const color = BRAND_COLOR_TABLE[colorDepth]
  return color ? color(str) : str
}
