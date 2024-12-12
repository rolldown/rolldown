import { imports } from "../../deno.json" with { type: "json" };
console.log(imports);

import * as denoJson from "@/deno.json" with { type: "json" };
console.log(denoJson);

import { fromFileUrl } from "jsr:@std/path@1.0.8/from-file-url";
const d = await import("jsr:@std/crypto");
const assert = await import("@std/assert");

import * as r3f from "@react-three/fiber";
import * as r3f2 from "https://esm.sh/zustand@5.0.2?external=react";

console.log(r3f, r3f2);

import * as noop from "npm:three";
console.log(noop);

import * as dep1 from "@/basic/dep1";
console.log(dep1);

import * as dep2 from "@/tests/jsr/dep1.ts";
console.log(dep2);

import * as dep3 from "../../tests/jsr/dep1.ts";
console.log(dep3);

const a = fromFileUrl(import.meta.resolve("./web.ts"));
console.log(a, d, assert);

import * as foo3 from "https://jsr.io/@rebeccastevens/rollup-plugin-dts/1.0.2/jsr.json" with {
  type: "json",
};
console.log(foo3);

import * as f5 from "npm:zod-form-data@^2.0.2";
console.log(f5);

import * as f3 from "npm:@floating-ui/dom@^1.6.12";
console.log(f3);

