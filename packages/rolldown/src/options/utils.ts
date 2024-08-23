import { BindingStringOrRegex } from "../binding.d";
import { isRegExp } from "node:util/types";

export function normalizedStringOrRegex(pattern?: Array<string  | RegExp> | (string | RegExp)): BindingStringOrRegex[] | undefined {
  if (!pattern) {
    return undefined;
  }
    if (isRegExp(pattern) || typeof pattern === 'string') {
      pattern = [pattern]
    }
    let ret: BindingStringOrRegex[] = []
  for (let p of pattern) {
      if (isRegExp(p)) {
        ret.push({ value: p.source, flag: p.flags })
      } else {
        ret.push({ value: p })
      }
  }
  return ret;
}
