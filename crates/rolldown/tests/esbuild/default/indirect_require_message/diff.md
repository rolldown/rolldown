# Reason
1. not align
# Diff
## /out/array.js
### esbuild
```js

```
### rolldown
```js
import { __require } from "./chunk.js";

//#region array.js
let x = [__require];

```
### diff
```diff
===================================================================
--- esbuild	/out/array.js
+++ rolldown	array.js
@@ -0,0 +1,2 @@
+import {__require} from "./chunk.js";
+var x = [__require];

```
## /out/assign.js
### esbuild
```js
// assign.js
__require = x;
```
### rolldown
```js
import { __require } from "./chunk.js";

//#region assign.js
require = x;

```
### diff
```diff
===================================================================
--- esbuild	/out/assign.js
+++ rolldown	assign.js
@@ -1,1 +1,2 @@
-__require = x;
+import {__require} from "./chunk.js";
+require = x;

```
## /out/dot.js
### esbuild
```js
// dot.js
var x = __require.cache;
```
### rolldown
```js
import { __require } from "./chunk.js";

//#region dot.js
let x = __require.cache;

```
### diff
```diff
===================================================================
--- esbuild	/out/dot.js
+++ rolldown	dot.js
@@ -1,1 +1,2 @@
+import {__require} from "./chunk.js";
 var x = __require.cache;

```
## /out/ident.js
### esbuild
```js

```
### rolldown
```js
import { __require } from "./chunk.js";

//#region ident.js
let x = __require;

```
### diff
```diff
===================================================================
--- esbuild	/out/ident.js
+++ rolldown	ident.js
@@ -0,0 +1,2 @@
+import {__require} from "./chunk.js";
+var x = __require;

```
## /out/index.js
### esbuild
```js
// index.js
var x = __require[cache];
```
### rolldown
```js
import { __require } from "./chunk.js";

//#region index.js
let x = __require[cache];

```
### diff
```diff
===================================================================
--- esbuild	/out/index.js
+++ rolldown	index.js
@@ -1,1 +1,2 @@
+import {__require} from "./chunk.js";
 var x = __require[cache];

```