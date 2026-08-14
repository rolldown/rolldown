class CustomError extends Error {
  constructor(message) {
    super(message);
    Object.setPrototypeOf(this, new.target.prototype);
  }
}

console.log(import.meta.url);

export { CustomError };
