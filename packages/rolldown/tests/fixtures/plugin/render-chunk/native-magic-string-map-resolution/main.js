export function parseAmount(text) {
  const amount = Number.parseFloat(text);
  return Number.isNaN(amount) ? undefined : amount;
}
