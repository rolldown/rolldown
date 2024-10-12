# Diff
## /out/entry.js
### esbuild
```js
// entry.js
var entry_default = () => {
  /**
   * @preserve
   */
};
export {
  entry_default as default
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
@@ -1,2 +0,0 @@
-var entry_default = () => {};
-export {entry_default as default};

```
## /out/entry.css
### esbuild
```js
/* entry.css */
@media (x: y) {
  /**
   * @preserve
   */
  z {
    zoom: 2;
  }
}
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/entry.css
+++ rolldown	
@@ -1,9 +0,0 @@
-/* entry.css */
-@media (x: y) {
-  /**
-   * @preserve
-   */
-  z {
-    zoom: 2;
-  }
-}
\ No newline at end of file

```