import { x } from './b.js';
function ignore(_value) {
  return 'called';
}
// The alias hides the callback from the `try` rule, so the walk of the callback checks the
// argument.
const attempt = (run) => {
  const call = run;
  try {
    call();
  } catch {
    result = 'tdz';
  }
};
let result;
attempt(() => {
  result = ignore(x);
});
export { result };
