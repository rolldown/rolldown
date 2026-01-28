// Shared helper utilities

export function formatDate(date) {
  return date.toISOString().split('T')[0];
}

export function formatTime(date) {
  return date.toISOString().split('T')[1].split('.')[0];
}

export function capitalize(str) {
  return str.charAt(0).toUpperCase() + str.slice(1);
}

export function debounce(fn, delay) {
  let timeoutId;
  return function (...args) {
    clearTimeout(timeoutId);
    timeoutId = setTimeout(() => fn.apply(this, args), delay);
  };
}
