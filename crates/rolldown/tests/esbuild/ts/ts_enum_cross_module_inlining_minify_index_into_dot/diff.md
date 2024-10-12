# Diff
## /out.js
### esbuild
```js
// entry.ts
inlined = [
  obj.abc,
  obj.xyz,
  obj?.abc,
  obj?.xyz,
  obj?.prop.abc,
  obj?.prop.xyz
];
notInlined = [
  obj["a b c" /* foo2 */],
  obj["x y z" /* bar2 */],
  obj?.["a b c" /* foo2 */],
  obj?.["x y z" /* bar2 */],
  obj?.prop["a b c" /* foo2 */],
  obj?.prop["x y z" /* bar2 */]
];
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	
@@ -1,2 +0,0 @@
-inlined = [obj.abc, obj.xyz, obj?.abc, obj?.xyz, obj?.prop.abc, obj?.prop.xyz];
-notInlined = [obj["a b c"], obj["x y z"], obj?.["a b c"], obj?.["x y z"], obj?.prop["a b c"], obj?.prop["x y z"]];

```