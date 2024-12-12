import { imports } from "../../deno.json" with {type: "json"};
console.log(imports);

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
