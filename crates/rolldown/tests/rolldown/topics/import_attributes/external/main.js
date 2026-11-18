import json from 'ext' with { type: 'json' };
import plain from 'ext';
import { named } from 'ext' with { type: 'json' };
import * as ns from 'ext-ns' with { type: 'css' };
export * from 'ext-ns' with { type: 'css' };
import 'ext' with { side: 'effect' };

export { json, plain, named, ns };
