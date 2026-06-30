import { getValue } from './parent.cjs';

export const POST = async () => {
  return new Response(getValue());
};
