const btn = document.querySelector('button');
let count = 0;
btn.onclick = () => {
  count++;
  btn.textContent = `Counter ${count}`;
};

// Referenced so the bundler keeps the dynamic edge, but never called: `dep.js` never
// executes in this tab.
function neverCalled() {
  import('./dep.js');
}
