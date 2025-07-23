import { foo } from "./foo.js";
import { bar } from "./bar.js";
import { prefix } from "./common.js";
import { prefix as p2 } from "./common.js";
import { prefix as p3 } from "./common.js";

export let msg = [prefix, foo, bar, p2, p3].join(",");
export function sayMessage() {
  console.log(msg);
}

if (import.meta.hot) {
  import.meta.hot.accept((mod) => {
    console.log("replaced with new msg: ", mod.msg);
    msg = mod.msg;
  });
}
