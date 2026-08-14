export const deadValue = 'dead-v1';

document.querySelector('.dead').textContent = deadValue;

// Statically visible to the compiler's scanner, but it never EXECUTES —
// so this module is NOT a runtime HMR boundary. An edit here must produce
// a clean full page reload, never a silently stale page.
if (false) {
  import.meta.hot?.accept();
}
