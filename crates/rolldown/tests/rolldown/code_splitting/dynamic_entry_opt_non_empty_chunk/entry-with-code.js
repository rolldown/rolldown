// This entry file has its own code in addition to importing from lib
import { libValue } from './lib';

// This code will make the chunk non-empty
const localValue = 10;

export const value = libValue + localValue;
