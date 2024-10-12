# Diff
## /out.js
### esbuild
```js
// entry.js
var keep;
var keep2;
keep + keep2;
keep - keep2;
keep * keep2;
keep / keep2;
keep % keep2;
keep ** keep2;
keep < keep2;
keep <= keep2;
keep > keep2;
keep >= keep2;
keep in keep2;
keep instanceof keep2;
keep << keep2;
keep >> keep2;
keep >>> keep2;
keep == keep2;
keep != keep2;
keep | keep2;
keep & keep2;
keep ^ keep2;
keep = keep2;
keep += keep2;
keep -= keep2;
keep *= keep2;
keep /= keep2;
keep %= keep2;
keep **= keep2;
keep <<= keep2;
keep >>= keep2;
keep >>>= keep2;
keep |= keep2;
keep &= keep2;
keep ^= keep2;
keep ??= keep2;
keep ||= keep2;
keep &&= keep2;
```
### rolldown
```js

//#region entry.js
let keep, keep2;
keep = keep2;
keep += keep2;
keep -= keep2;
keep *= keep2;
keep /= keep2;
keep %= keep2;
keep **= keep2;
keep <<= keep2;
keep >>= keep2;
keep >>>= keep2;
keep |= keep2;
keep &= keep2;
keep ^= keep2;
keep ??= keep2;
keep ||= keep2;
keep &&= keep2;

//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,26 +1,5 @@
-var keep;
-var keep2;
-keep + keep2;
-keep - keep2;
-keep * keep2;
-keep / keep2;
-keep % keep2;
-keep ** keep2;
-keep < keep2;
-keep <= keep2;
-keep > keep2;
-keep >= keep2;
-(keep in keep2);
-keep instanceof keep2;
-keep << keep2;
-keep >> keep2;
-keep >>> keep2;
-keep == keep2;
-keep != keep2;
-keep | keep2;
-keep & keep2;
-keep ^ keep2;
+var keep, keep2;
 keep = keep2;
 keep += keep2;
 keep -= keep2;
 keep *= keep2;

```