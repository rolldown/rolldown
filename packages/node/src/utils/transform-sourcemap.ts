import type {
    SourceMap
} from '@rolldown/node-binding'

import type {
    SourceMapInput
} from '../rollup-types'


export function transformSourcemap(value?: SourceMapInput): SourceMap | undefined {
    if (value === undefined || value === null) {
        return
    }
    if (typeof value === 'string') {
        return JSON.parse(value) as SourceMap
    }
    if (typeof value === 'object') {
        return {
            // TODO file, version, x_google_ignoreList
            mappings: value.mappings,
            names: 'names' in value ? value.names : [],
            sourceRoot: 'sourceRoot' in value ? value.sourceRoot : "",
            sources: 'sources' in value ? value.sources : [],
            sourcesContent: 'sourcesContent' in value ? value.sourcesContent ? value.sourcesContent.filter((v) => typeof v === 'string') as string [] : [] : [],
        }
    }
}