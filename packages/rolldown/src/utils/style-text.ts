/**
 * Cross-platform styleText utility that works in both Node.js and browser environments
 * In Node.js, it uses the native `styleText` from `node:util`
 * In browser, it provides empty styling functions for compatibility
 */
import { styleText as nodeStyleText } from 'node:util';

type Args = typeof nodeStyleText extends (...arg: infer U) => any ? U : never;

export function styleText(...args: Args): string {
  if (import.meta.browserBuild === true) {
    return args[1];
  } else {
    return nodeStyleText(...args);
  }
}
