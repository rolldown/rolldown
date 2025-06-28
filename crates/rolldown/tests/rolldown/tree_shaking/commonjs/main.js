// import assert from "node:assert";
import * as ns from "./src/basic_ns/";
import default_cjs from "./src/basic_ref_with_named_default";
//
import * as export_star_from_cjs from "./src/export_star_from_cjs/";
import {another as another1} from "./src/nested_export_star_from_cjs/";

import { a } from "./src/named_import_export_star_from_cjs/";

console.log(ns.a, "basic-a");
// //
assert.equal(default_cjs.a, "basic_ref_with_named_default_a");

console.log(export_star_from_cjs.a, "export_star_from_cjs_a");

console.log(a, "named_import_export_star_from_cjs_a");

console.log(another1.a, "named_import_export_star_from_cjs_a");
