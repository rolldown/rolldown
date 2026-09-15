# delete_file_used

Deleting a file that is still imported, then recreating it.

| Step | Edit                                         | Update type |
| ---- | -------------------------------------------- | ----------- |
| 0    | delete `child.js`                            | `noop`      |
| 1    | recreate `child.js` with **changed** content | `patch`     |

Step 0 is a `noop` on purpose: the importer (`parent.js`) is re-scanned, but its rebuilt output is byte-identical (the module graph still serves the cached `child.js`), so there is nothing to ship. Step 1 recreates the file with different content, which must ship a patch and re-run `parent.js` — the `beforeExit` guard in `main.js` fails loudly if it does not.
