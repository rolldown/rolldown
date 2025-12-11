const num = 0;
const config = (num) => num;

console.log(config(42)); // Should log 42, not 0
console.log(num); // Should log 0

export { config, num };
