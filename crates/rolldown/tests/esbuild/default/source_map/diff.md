# Diff
## /Users/user/project/out.js.map
### esbuild
```js
{
  "version": 3,
  "sources": ["src/bar.js", "src/data.txt", "src/entry.js"],
  "sourcesContent": ["\n\t\t\t\texport function bar() { throw new Error('test') }\n\t\t\t", "#2041", "\n\t\t\t\timport {bar} from './bar'\n\t\t\t\timport data from './data.txt'\n\t\t\t\tfunction foo() { bar() }\n\t\t\t\tfoo()\n\t\t\t\tconsole.log(data)\n\t\t\t"],
  "mappings": ";AACW,SAAS,MAAM;AAAE,QAAM,IAAI,MAAM,MAAM;AAAE;;;ACDpD;;;ACGI,SAAS,MAAM;AAAE,MAAI;AAAE;AACvB,IAAI;AACJ,QAAQ,IAAI,YAAI;",
  "names": []
}
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/Users/user/project/out.js.map
+++ rolldown	
@@ -1,7 +0,0 @@
-{
-  "version": 3,
-  "sources": ["src/bar.js", "src/data.txt", "src/entry.js"],
-  "sourcesContent": ["\n\t\t\t\texport function bar() { throw new Error('test') }\n\t\t\t", "#2041", "\n\t\t\t\timport {bar} from './bar'\n\t\t\t\timport data from './data.txt'\n\t\t\t\tfunction foo() { bar() }\n\t\t\t\tfoo()\n\t\t\t\tconsole.log(data)\n\t\t\t"],
-  "mappings": ";AACW,SAAS,MAAM;AAAE,QAAM,IAAI,MAAM,MAAM;AAAE;;;ACDpD;;;ACGI,SAAS,MAAM;AAAE,MAAI;AAAE;AACvB,IAAI;AACJ,QAAQ,IAAI,YAAI;",
-  "names": []
-}
\ No newline at end of file

```
## /Users/user/project/out.js
### esbuild
```js
// Users/user/project/src/bar.js
function bar() {
  throw new Error("test");
}

// Users/user/project/src/data.txt
var data_default = "#2041";

// Users/user/project/src/entry.js
function foo() {
  bar();
}
foo();
console.log(data_default);
//# sourceMappingURL=out.js.map
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/Users/user/project/out.js
+++ rolldown	
@@ -1,9 +0,0 @@
-function bar() {
-    throw new Error("test");
-}
-var data_default = "#2041";
-function foo() {
-    bar();
-}
-foo();
-console.log(data_default);

```