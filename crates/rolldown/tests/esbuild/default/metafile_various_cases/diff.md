# Diff
## /out/file-NVISQQTV.file
### esbuild
```js
file
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/file-NVISQQTV.file
+++ rolldown	
@@ -1,1 +0,0 @@
-file;

```
## /out/copy-O3Y5SCJE.copy
### esbuild
```js
copy
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/copy-O3Y5SCJE.copy
+++ rolldown	
@@ -1,1 +0,0 @@
-copy;

```
## /out/entry.js
### esbuild
```js
import {
  __commonJS,
  __require
} from "./chunk-MQN2VSL5.js";

// project/cjs.js
var require_cjs = __commonJS({
  "project/cjs.js"(exports, module) {
    module.exports = 4;
  }
});

// project/entry.js
import a from "extern-esm";

// project/esm.js
var esm_default = 1;

// <data:application/json,2>
var json_2_default = 2;

// project/file.file
var file_default = "./file-NVISQQTV.file";

// project/entry.js
import e from "./copy-O3Y5SCJE.copy";
console.log(
  a,
  esm_default,
  json_2_default,
  file_default,
  e,
  __require("extern-cjs"),
  require_cjs(),
  import("./dynamic-Q2DWDUFV.js")
);
var exported;
export {
  exported
};
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/entry.js
+++ rolldown	
@@ -1,14 +0,0 @@
-import {__commonJS, __require} from "./chunk-MQN2VSL5.js";
-var require_cjs = __commonJS({
-    "project/cjs.js"(exports, module) {
-        module.exports = 4;
-    }
-});
-import a from "extern-esm";
-var esm_default = 1;
-var json_2_default = 2;
-var file_default = "./file-NVISQQTV.file";
-import e from "./copy-O3Y5SCJE.copy";
-console.log(a, esm_default, json_2_default, file_default, e, __require("extern-cjs"), require_cjs(), import("./dynamic-Q2DWDUFV.js"));
-var exported;
-export {exported};

```
## /out/dynamic-Q2DWDUFV.js
### esbuild
```js
import "./chunk-MQN2VSL5.js";

// project/dynamic.js
var dynamic_default = 5;
export {
  dynamic_default as default
};
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/dynamic-Q2DWDUFV.js
+++ rolldown	
@@ -1,3 +0,0 @@
-import "./chunk-MQN2VSL5.js";
-var dynamic_default = 5;
-export {dynamic_default as default};

```