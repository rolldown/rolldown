# Reason
1. var decl not join, trivial
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
// These operators may have side effects
let keep, keep2;
keep + keep2;
keep - keep2;
keep * keep2;
keep / keep2;
keep % keep2;
keep ** keep2;
keep in keep2;
keep instanceof keep2;
keep << keep2;
keep >> keep2;
keep >>> keep2;
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

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,23 +1,16 @@
-var keep;
-var keep2;
+var keep, keep2;
 keep + keep2;
 keep - keep2;
 keep * keep2;
 keep / keep2;
 keep % keep2;
 keep ** keep2;
-keep < keep2;
-keep <= keep2;
-keep > keep2;
-keep >= keep2;
 (keep in keep2);
 keep instanceof keep2;
 keep << keep2;
 keep >> keep2;
 keep >>> keep2;
-keep == keep2;
-keep != keep2;
 keep | keep2;
 keep & keep2;
 keep ^ keep2;
 keep = keep2;

```