# delete_file_used

Deleting a file that is still imported, then recreating it.

| Step | Edit                                         | Update type                 |
| ---- | -------------------------------------------- | --------------------------- |
| 0    | delete `child.js`                            | error (`UNRESOLVED_IMPORT`) |
| 1    | recreate `child.js` with **changed** content | `patch`                     |

Step 0 fails with `UNRESOLVED_IMPORT`, the same result as a cold build of that file state. The delete clears the resolver cache, so the re-scan of the importer (`parent.js`) cannot resolve `./child.js` from a stale cache entry. Step 1 recreates the file with different content. The harness reports it as an update, not a create, so recovery relies on the cache also being cleared after a failed task. It must ship a patch and re-run `parent.js`. The `beforeExit` guard in `main.js` fails loudly if it does not. See https://github.com/rolldown/rolldown/issues/10487.
