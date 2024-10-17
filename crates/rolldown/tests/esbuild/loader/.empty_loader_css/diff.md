# Reason
1. css stabilization
# Diff
## entry.css.map
### esbuild
```js
{
  "version": 3,
  "sources": ["entry.css"],
  "sourcesContent": ["\n\t\t\t\t@import 'a.empty';\n\t\t\t\ta { background: url(b.empty) }\n\t\t\t"],
  "mappings": ";AAEI;AAAI,cAAY;AAAa;",
  "names": []
}
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	entry.css.map
+++ rolldown	
@@ -1,7 +0,0 @@
-{
-  "version": 3,
-  "sources": ["entry.css"],
-  "sourcesContent": ["\n\t\t\t\t@import 'a.empty';\n\t\t\t\ta { background: url(b.empty) }\n\t\t\t"],
-  "mappings": ";AAEI;AAAI,cAAY;AAAa;",
-  "names": []
-}
\ No newline at end of file

```
## entry.css
### esbuild
```js
/* entry.css */
a {
  background: url();
}
```
### rolldown
```js
@import 'a.empty';
a { background: url(b.empty) }

```
### diff
```diff
===================================================================
--- esbuild	entry.css
+++ rolldown	entry.css
@@ -1,4 +1,2 @@
-/* entry.css */
-a {
-  background: url();
-}
\ No newline at end of file
+@import 'a.empty';
+a { background: url(b.empty) }

```
## metafile.json
### esbuild
```js
{
  "inputs": {
    "a.empty": {
      "bytes": 0,
      "imports": []
    },
    "b.empty": {
      "bytes": 0,
      "imports": []
    },
    "entry.css": {
      "bytes": 62,
      "imports": [
        {
          "path": "a.empty",
          "kind": "import-rule",
          "original": "a.empty"
        },
        {
          "path": "b.empty",
          "kind": "url-token",
          "original": "b.empty"
        }
      ]
    }
  },
  "outputs": {
    "entry.css.map": {
      "imports": [],
      "exports": [],
      "inputs": {},
      "bytes": 203
    },
    "entry.css": {
      "imports": [
        {
          "path": "",
          "kind": "url-token",
          "external": true
        }
      ],
      "entryPoint": "entry.css",
      "inputs": {
        "entry.css": {
          "bytesInOutput": 27
        }
      },
      "bytes": 43
    }
  }
}
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	metafile.json
+++ rolldown	
@@ -1,51 +0,0 @@
-{
-  "inputs": {
-    "a.empty": {
-      "bytes": 0,
-      "imports": []
-    },
-    "b.empty": {
-      "bytes": 0,
-      "imports": []
-    },
-    "entry.css": {
-      "bytes": 62,
-      "imports": [
-        {
-          "path": "a.empty",
-          "kind": "import-rule",
-          "original": "a.empty"
-        },
-        {
-          "path": "b.empty",
-          "kind": "url-token",
-          "original": "b.empty"
-        }
-      ]
-    }
-  },
-  "outputs": {
-    "entry.css.map": {
-      "imports": [],
-      "exports": [],
-      "inputs": {},
-      "bytes": 203
-    },
-    "entry.css": {
-      "imports": [
-        {
-          "path": "",
-          "kind": "url-token",
-          "external": true
-        }
-      ],
-      "entryPoint": "entry.css",
-      "inputs": {
-        "entry.css": {
-          "bytesInOutput": 27
-        }
-      },
-      "bytes": 43
-    }
-  }
-}
\ No newline at end of file

```