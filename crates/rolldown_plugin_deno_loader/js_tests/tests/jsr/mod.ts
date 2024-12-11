import { fromFileUrl } from "jsr:@std/path@1.0.8/from-file-url";
const d = await import("jsr:@std/crypto");
const assert = await import("@std/assert");

import * as noop from "npm:three";
console.log(noop);

const a = fromFileUrl(import.meta.resolve("./web.ts"));
console.log(a, d, assert);
