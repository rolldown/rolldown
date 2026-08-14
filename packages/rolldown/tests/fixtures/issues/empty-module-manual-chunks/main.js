// Top-level await inlines the import (no separate chunk created),
// so a function wrapper is needed to trigger the cross-chunk export path.
(async () => {
  await import('fake-pkg/empty');
})();
