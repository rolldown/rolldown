import assert from "node:assert";

import * as ns from "./src/basic_ns/";
import default_cjs from "./src/basic_ref_with_named_default";
import * as export_star_from_cjs from "./src/export_star_from_cjs/";
import { a } from "./src/named_import_export_star_from_cjs/";
import {another as another1} from "./src/nested_export_star_from_cjs/";
import react from './src/indirect_common_js/'

assert.equal(ns.a, "basic-a");
assert.equal(ns.a.startsWith("b"), true);
assert.equal(default_cjs.a, "basic-ref-with-named-default-a");
assert.equal(default_cjs.a.startsWith("b"),true);

assert.equal(export_star_from_cjs.a, "export-star-from-cjs-a");
assert.equal(export_star_from_cjs.a, "export-star-from-cjs-a");

assert.equal(a, "named-import-export-star-from-cjs-a");
assert.equal(a.startsWith("n"), true);

assert.equal(another1.a, "nested-export-star-from-cjs-a");
assert.equal(another1.a.startsWith("n"), true);

assert.equal(react.a, 'react-like-a')
assert.equal(react.a.startsWith("r"), true)
