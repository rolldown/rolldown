export let send = () => {};

export const setup = () => {
  send = (msg) => {
    globalThis.result = msg;
  };
};
