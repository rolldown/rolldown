# Diff
## /out/esm.js
### esbuild
```js
export let foo_ = 123;
import { bar_ } from "xyz";
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/esm.js
+++ rolldown	
@@ -1,2 +0,0 @@
-export let foo_ = 123;
-import {bar_} from "xyz";

```
## /out/cjs.js
### esbuild
```js
exports.a = 123;
let bar_ = require("xyz").b;
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/cjs.js
+++ rolldown	
@@ -1,2 +0,0 @@
-exports.a = 123;
-let bar_ = require("xyz").b;

```