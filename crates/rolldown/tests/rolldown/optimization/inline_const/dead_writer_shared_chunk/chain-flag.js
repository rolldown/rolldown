// Only a function that nothing calls calls the writer.
export let chained = false;

function enable_chained() {
  chained = true;
}

function setup() {
  enable_chained();
}
