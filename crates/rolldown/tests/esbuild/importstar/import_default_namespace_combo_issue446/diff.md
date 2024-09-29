## /out/external-default2.js
### esbuild
```js
// external-default2.js
import def, { default as default2 } from "external";
console.log(def, default2);
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/external-default2.js
+++ rolldown	
@@ -1,2 +0,0 @@
-import def, {default as default2} from "external";
-console.log(def, default2);

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

```
### diff
```diff
===================================================================
--- esbuild	/out/external-ns.js
+++ rolldown	
@@ -1,2 +0,0 @@
-import def, * as ns from "external";
-console.log(def, ns);

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

```
### diff
```diff
===================================================================
--- esbuild	/out/external-ns-default.js
+++ rolldown	
@@ -1,2 +0,0 @@
-import def, * as ns from "external";
-console.log(def, ns, ns.default);

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

```
### diff
```diff
===================================================================
--- esbuild	/out/external-ns-def.js
+++ rolldown	
@@ -1,2 +0,0 @@
-import def, * as ns from "external";
-console.log(def, ns, ns.def);

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

```
### diff
```diff
===================================================================
--- esbuild	/out/external-default.js
+++ rolldown	
@@ -1,2 +0,0 @@
-import def, * as ns from "external";
-console.log(def, ns.default);

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

```
### diff
```diff
===================================================================
--- esbuild	/out/external-def.js
+++ rolldown	
@@ -1,2 +0,0 @@
-import def, * as ns from "external";
-console.log(def, ns.def);

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

```
### diff
```diff
===================================================================
--- esbuild	/out/internal-default2.js
+++ rolldown	
@@ -1,2 +0,0 @@
-var internal_default = 123;
-console.log(internal_default, internal_default);

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

```
### diff
```diff
===================================================================
--- esbuild	/out/internal-ns.js
+++ rolldown	
@@ -1,6 +0,0 @@
-var internal_exports = {};
-__export(internal_exports, {
-    default: () => internal_default
-});
-var internal_default = 123;
-console.log(internal_default, internal_exports);

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

```
### diff
```diff
===================================================================
--- esbuild	/out/internal-ns-default.js
+++ rolldown	
@@ -1,6 +0,0 @@
-var internal_exports = {};
-__export(internal_exports, {
-    default: () => internal_default
-});
-var internal_default = 123;
-console.log(internal_default, internal_exports, internal_default);

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

```
### diff
```diff
===================================================================
--- esbuild	/out/internal-ns-def.js
+++ rolldown	
@@ -1,6 +0,0 @@
-var internal_exports = {};
-__export(internal_exports, {
-    default: () => internal_default
-});
-var internal_default = 123;
-console.log(internal_default, internal_exports, void 0);

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

```
### diff
```diff
===================================================================
--- esbuild	/out/internal-default.js
+++ rolldown	
@@ -1,2 +0,0 @@
-var internal_default = 123;
-console.log(internal_default, internal_default);

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

```
### diff
```diff
===================================================================
--- esbuild	/out/internal-def.js
+++ rolldown	
@@ -1,2 +0,0 @@
-var internal_default = 123;
-console.log(internal_default, void 0);

```
