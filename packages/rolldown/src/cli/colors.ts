import { Color, createColors } from 'colorette';
import { env } from 'node:process';

// @see https://no-color.org
// @see https://www.npmjs.com/package/chalk
const {
  bold,
  cyan,
  dim,
  gray,
  green,
  red,
  underline,
  yellow,
}: {
  bold: Color;
  cyan: Color;
  dim: Color;
  gray: Color;
  green: Color;
  red: Color;
  underline: Color;
  yellow: Color;
} = createColors({
  useColor: env.FORCE_COLOR !== '0' && !env.NO_COLOR,
});

export const colors: {
  bold: Color;
  cyan: Color;
  dim: Color;
  gray: Color;
  green: Color;
  red: Color;
  underline: Color;
  yellow: Color;
} = {
  bold,
  cyan,
  dim,
  gray,
  green,
  red,
  underline,
  yellow,
};
