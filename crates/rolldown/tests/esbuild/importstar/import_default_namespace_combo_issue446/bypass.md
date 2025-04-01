# Reason
1. Rolldown extract common chunk
2. Rolldown try to merge external default import binding
# Diff
## /out/external-default2.js
### esbuild
```js
// external-default2.js
import def, { default as default2 } from "external";
console.log(def, default2);
```
### rolldown
```js
import def from "external";

//#region external-default2.js
console.log(def, def);
//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out/external-default2.js
+++ rolldown	external-default2.js
@@ -1,2 +1,2 @@
-import def, {default as default2} from "external";
-console.log(def, default2);
+import def from "external";
+console.log(def, def);

```
## /out/external-ns.js
### esbuild
```js
// external-ns.js
import def, * as ns from "external";
console.log(def, ns);
```
### rolldown
```js
import * as ns from "external";
import def from "external";

//#region external-ns.js
console.log(def, ns);
//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out/external-ns.js
+++ rolldown	external-ns.js
@@ -1,2 +1,3 @@
-import def, * as ns from "external";
+import * as ns from "external";
+import def from "external";
 console.log(def, ns);

```
## /out/external-ns-default.js
### esbuild
```js
// external-ns-default.js
import def, * as ns from "external";
console.log(def, ns, ns.default);
```
### rolldown
```js
import * as ns from "external";
import def from "external";

//#region external-ns-default.js
console.log(def, ns, ns.default);
//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out/external-ns-default.js
+++ rolldown	external-ns-default.js
@@ -1,2 +1,3 @@
-import def, * as ns from "external";
+import * as ns from "external";
+import def from "external";
 console.log(def, ns, ns.default);

```
## /out/external-ns-def.js
### esbuild
```js
// external-ns-def.js
import def, * as ns from "external";
console.log(def, ns, ns.def);
```
### rolldown
```js
import * as ns from "external";
import def from "external";

//#region external-ns-def.js
console.log(def, ns, ns.def);
//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out/external-ns-def.js
+++ rolldown	external-ns-def.js
@@ -1,2 +1,3 @@
-import def, * as ns from "external";
+import * as ns from "external";
+import def from "external";
 console.log(def, ns, ns.def);

```
## /out/external-default.js
### esbuild
```js
// external-default.js
import def, * as ns from "external";
console.log(def, ns.default);
```
### rolldown
```js
import * as ns from "external";
import def from "external";

//#region external-default.js
console.log(def, ns.default);
//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out/external-default.js
+++ rolldown	external-default.js
@@ -1,2 +1,3 @@
-import def, * as ns from "external";
+import * as ns from "external";
+import def from "external";
 console.log(def, ns.default);

```
## /out/external-def.js
### esbuild
```js
// external-def.js
import def, * as ns from "external";
console.log(def, ns.def);
```
### rolldown
```js
import * as ns from "external";
import def from "external";

//#region external-def.js
console.log(def, ns.def);
//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out/external-def.js
+++ rolldown	external-def.js
@@ -1,2 +1,3 @@
-import def, * as ns from "external";
+import * as ns from "external";
+import def from "external";
 console.log(def, ns.def);

```
## /out/internal-default2.js
### esbuild
```js
// internal.js
var internal_default = 123;

// internal-default2.js
console.log(internal_default, internal_default);
```
### rolldown
```js
import { internal_default } from "./internal.js";

//#region internal-default2.js
console.log(internal_default, internal_default);
//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out/internal-default2.js
+++ rolldown	internal-default2.js
@@ -1,2 +1,2 @@
-var internal_default = 123;
+import {internal_default} from "./internal.js";
 console.log(internal_default, internal_default);

```
## /out/internal-ns.js
### esbuild
```js
// internal.js
var internal_exports = {};
__export(internal_exports, {
  default: () => internal_default
});
var internal_default = 123;

// internal-ns.js
console.log(internal_default, internal_exports);
```
### rolldown
```js
import { internal_default, internal_exports } from "./internal.js";

//#region internal-ns.js
console.log(internal_default, internal_exports);
//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out/internal-ns.js
+++ rolldown	internal-ns.js
@@ -1,6 +1,2 @@
-var internal_exports = {};
-__export(internal_exports, {
-    default: () => internal_default
-});
-var internal_default = 123;
+import {internal_default, internal_exports} from "./internal.js";
 console.log(internal_default, internal_exports);

```
## /out/internal-ns-default.js
### esbuild
```js
// internal.js
var internal_exports = {};
__export(internal_exports, {
  default: () => internal_default
});
var internal_default = 123;

// internal-ns-default.js
console.log(internal_default, internal_exports, internal_default);
```
### rolldown
```js
import { internal_default, internal_exports } from "./internal.js";

//#region internal-ns-default.js
console.log(internal_default, internal_exports, internal_default);
//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out/internal-ns-default.js
+++ rolldown	internal-ns-default.js
@@ -1,6 +1,2 @@
-var internal_exports = {};
-__export(internal_exports, {
-    default: () => internal_default
-});
-var internal_default = 123;
+import {internal_default, internal_exports} from "./internal.js";
 console.log(internal_default, internal_exports, internal_default);

```
## /out/internal-ns-def.js
### esbuild
```js
// internal.js
var internal_exports = {};
__export(internal_exports, {
  default: () => internal_default
});
var internal_default = 123;

// internal-ns-def.js
console.log(internal_default, internal_exports, void 0);
```
### rolldown
```js
import { internal_default, internal_exports } from "./internal.js";

//#region internal-ns-def.js
console.log(internal_default, internal_exports, void 0);
//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out/internal-ns-def.js
+++ rolldown	internal-ns-def.js
@@ -1,6 +1,2 @@
-var internal_exports = {};
-__export(internal_exports, {
-    default: () => internal_default
-});
-var internal_default = 123;
+import {internal_default, internal_exports} from "./internal.js";
 console.log(internal_default, internal_exports, void 0);

```
## /out/internal-default.js
### esbuild
```js
// internal.js
var internal_default = 123;

// internal-default.js
console.log(internal_default, internal_default);
```
### rolldown
```js
import { internal_default } from "./internal.js";

//#region internal-default.js
console.log(internal_default, internal_default);
//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out/internal-default.js
+++ rolldown	internal-default.js
@@ -1,2 +1,2 @@
-var internal_default = 123;
+import {internal_default} from "./internal.js";
 console.log(internal_default, internal_default);

```
## /out/internal-def.js
### esbuild
```js
// internal.js
var internal_default = 123;

// internal-def.js
console.log(internal_default, void 0);
```
### rolldown
```js
import { internal_default } from "./internal.js";

//#region internal-def.js
console.log(internal_default, void 0);
//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out/internal-def.js
+++ rolldown	internal-def.js
@@ -1,2 +1,2 @@
-var internal_default = 123;
+import {internal_default} from "./internal.js";
 console.log(internal_default, void 0);

```