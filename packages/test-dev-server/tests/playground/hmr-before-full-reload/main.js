export const value = 'reload-v1';
document.querySelector('.value').textContent = value;

// No accept anywhere: editing this module makes the client walk find no boundary and
// reload the page itself. The event must fire before that reload; sessionStorage
// survives it where window state does not.
import.meta.hot?.on('vite:beforeFullReload', () => {
  sessionStorage.setItem('sawBeforeFullReload', '1');
});
