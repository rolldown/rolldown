import { BindingTransformPluginContext } from '@src/binding'
import { SourceMap } from '@src/types/rolldown-output'

export class TransformPluginContext {
  getCombinedSourcemap: () => SourceMap

  constructor(context: BindingTransformPluginContext) {
    this.getCombinedSourcemap = () => JSON.parse(context.getCombinedSourcemap())
  }
}
