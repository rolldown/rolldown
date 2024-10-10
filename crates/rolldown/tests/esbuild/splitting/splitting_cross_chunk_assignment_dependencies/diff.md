## /out/a.js
### esbuild
```js
import {
  setValue
} from "./chunk-3GNPIT25.js";

// a.js
setValue(123);
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/a.js
+++ rolldown	
@@ -1,2 +0,0 @@
-import {setValue} from "./chunk-3GNPIT25.js";
-setValue(123);

```
## /out/b.js
### esbuild
```js
import "./chunk-3GNPIT25.js";
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/b.js
+++ rolldown	
@@ -1,1 +0,0 @@
-import "./chunk-3GNPIT25.js";

```
## /out/chunk-3GNPIT25.js
### esbuild
```js
// shared.js
var observer;
var value;
function getValue() {
  return value;
}
function setValue(next) {
  value = next;
  if (observer) observer();
}
sideEffects(getValue);

export {
  setValue
};
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/chunk-3GNPIT25.js
+++ rolldown	
@@ -1,11 +0,0 @@
-var observer;
-var value;
-function getValue() {
-    return value;
-}
-function setValue(next) {
-    value = next;
-    if (observer) observer();
-}
-sideEffects(getValue);
-export {setValue};

```
# Diff
## /out/a.js
### esbuild
```js
import {
  setValue
} from "./chunk-3GNPIT25.js";

// a.js
setValue(123);
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/a.js
+++ rolldown	
@@ -1,2 +0,0 @@
-import {setValue} from "./chunk-3GNPIT25.js";
-setValue(123);

```
## /out/b.js
### esbuild
```js
import "./chunk-3GNPIT25.js";
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/b.js
+++ rolldown	
@@ -1,1 +0,0 @@
-import "./chunk-3GNPIT25.js";

```
## /out/chunk-3GNPIT25.js
### esbuild
```js
// shared.js
var observer;
var value;
function getValue() {
  return value;
}
function setValue(next) {
  value = next;
  if (observer) observer();
}
sideEffects(getValue);

export {
  setValue
};
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/chunk-3GNPIT25.js
+++ rolldown	
@@ -1,11 +0,0 @@
-var observer;
-var value;
-function getValue() {
-    return value;
-}
-function setValue(next) {
-    value = next;
-    if (observer) observer();
-}
-sideEffects(getValue);
-export {setValue};

```