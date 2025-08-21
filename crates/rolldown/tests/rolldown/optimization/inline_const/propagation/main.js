import { bar } from "./bar";
import foo from "./foo";

export function main() {
  if (foo) {
    console.warn("I expected this warning to be removed in the final bundle.");
  }

  console.log(bar);
}
