import { expect } from 'vitest'
import { defineTest } from '@tests'

const preName = 'test-plugin-pre'
const normalName = 'test-plugin-normal'
const postName = 'test-plugin-post'

// TODO
// The `buildStart` is run in parallel, so the order is not stable. If we need to implement `sequential`, we need to care the order.
// The `resolveDynamicImport/renderError` not tested.

const expectedCallsResult = [preName, normalName, postName]

const resolveIdCalls: string[] = []
const buildStartCalls: string[] = []
const renderChunkCalls: string[] = []
const buildEndCalls: string[] = []
const transformCalls: string[] = []
const moduleParsedCalls: string[] = []
const loadCalls: string[] = []
const augmentChunkHashCalls: string[] = []
const renderStartCalls: string[] = []
const generateBundleCalls: string[] = []
const writeBundleCalls: string[] = []
const bannerCalls: string[] = []
const footerCalls: string[] = []
const introCalls: string[] = []
const outroCalls: string[] = []

export default defineTest({
  config: {
    plugins: [
      {
        name: postName,
        buildStart: {
          handler: () => {
            buildStartCalls.push(postName)
          },
          order: 'post',
        },
        resolveId: {
          handler: () => {
            resolveIdCalls.push(postName)
          },
          order: 'post',
        },
        buildEnd: {
          handler: () => {
            buildEndCalls.push(postName)
          },
          order: 'post',
        },
        transform: {
          handler: () => {
            transformCalls.push(postName)
          },
          order: 'post',
        },
        moduleParsed: {
          handler: () => {
            moduleParsedCalls.push(postName)
          },
          order: 'post',
        },
        load: {
          handler: () => {
            loadCalls.push(postName)
          },
          order: 'post',
        },
        renderChunk: {
          handler: () => {
            renderChunkCalls.push(postName)
          },
          order: 'post',
        },
        augmentChunkHash: {
          handler: () => {
            augmentChunkHashCalls.push(postName)
          },
          order: 'post',
        },
        renderStart: {
          handler: () => {
            renderStartCalls.push(postName)
          },
          order: 'post',
        },
        generateBundle: {
          handler: () => {
            generateBundleCalls.push(postName)
          },
          order: 'post',
        },
        writeBundle: {
          handler: () => {
            writeBundleCalls.push(postName)
          },
          order: 'post',
        },
        banner: {
          handler: () => {
            bannerCalls.push(postName)
            return ''
          },
          order: 'post',
        },
        footer: {
          handler: () => {
            footerCalls.push(postName)
            return ''
          },
          order: 'post',
        },
        intro: {
          handler: () => {
            introCalls.push(postName)
            return ''
          },
          order: 'post',
        },
        outro: {
          handler: () => {
            outroCalls.push(postName)
            return ''
          },
          order: 'post',
        },
      },
      {
        name: preName,
        buildStart: {
          handler: () => {
            buildStartCalls.push(preName)
          },
          order: 'pre',
        },
        resolveId: {
          handler: () => {
            resolveIdCalls.push(preName)
          },
          order: 'pre',
        },
        buildEnd: {
          handler: () => {
            buildEndCalls.push(preName)
          },
          order: 'pre',
        },
        transform: {
          handler: () => {
            transformCalls.push(preName)
          },
          order: 'pre',
        },
        moduleParsed: {
          handler: () => {
            moduleParsedCalls.push(preName)
          },
          order: 'pre',
        },
        load: {
          handler: () => {
            loadCalls.push(preName)
          },
          order: 'pre',
        },
        renderChunk: {
          handler: () => {
            renderChunkCalls.push(preName)
          },
          order: 'pre',
        },
        augmentChunkHash: {
          handler: () => {
            augmentChunkHashCalls.push(preName)
          },
          order: 'pre',
        },
        renderStart: {
          handler: () => {
            renderStartCalls.push(preName)
          },
          order: 'pre',
        },
        generateBundle: {
          handler: () => {
            generateBundleCalls.push(preName)
          },
          order: 'pre',
        },
        writeBundle: {
          handler: () => {
            writeBundleCalls.push(preName)
          },
          order: 'pre',
        },
        banner: {
          handler: () => {
            bannerCalls.push(preName)
            return ''
          },
          order: 'pre',
        },
        footer: {
          handler: () => {
            footerCalls.push(preName)
            return ''
          },
          order: 'pre',
        },
        intro: {
          handler: () => {
            introCalls.push(preName)
            return ''
          },
          order: 'pre',
        },
        outro: {
          handler: () => {
            outroCalls.push(preName)
            return ''
          },
          order: 'pre',
        },
      },
      {
        name: normalName,
        buildStart: () => {
          buildStartCalls.push(normalName)
        },
        resolveId: () => {
          resolveIdCalls.push(normalName)
        },
        buildEnd: () => {
          buildEndCalls.push(normalName)
        },
        transform: () => {
          transformCalls.push(normalName)
        },
        moduleParsed: () => {
          moduleParsedCalls.push(normalName)
        },
        load: () => {
          loadCalls.push(normalName)
        },
        renderChunk: () => {
          renderChunkCalls.push(normalName)
        },
        augmentChunkHash: () => {
          augmentChunkHashCalls.push(normalName)
        },
        renderStart: () => {
          renderStartCalls.push(normalName)
        },
        generateBundle: () => {
          generateBundleCalls.push(normalName)
        },
        writeBundle: () => {
          writeBundleCalls.push(normalName)
        },
        banner: () => {
          bannerCalls.push(normalName)
          return ''
        },
        footer: () => {
          footerCalls.push(normalName)
          return ''
        },
        intro: () => {
          introCalls.push(normalName)
          return ''
        },
        outro: () => {
          outroCalls.push(normalName)
          return ''
        },
      },
    ],
  },
  skipComposingJsPlugin: true,
  beforeTest: () => {
    resolveIdCalls.length = 0
    // buildStartCalls.length = 0
    renderChunkCalls.length = 0
    buildEndCalls.length = 0
    resolveIdCalls.length = 0
    transformCalls.length = 0
    moduleParsedCalls.length = 0
    loadCalls.length = 0
    augmentChunkHashCalls.length = 0
    renderStartCalls.length = 0
    generateBundleCalls.length = 0
    writeBundleCalls.length = 0
    bannerCalls.length = 0
    footerCalls.length = 0
    introCalls.length = 0
    outroCalls.length = 0
  },
  afterTest: () => {
    expect(resolveIdCalls).toStrictEqual(expectedCallsResult)
    // expect(buildStartCalls).toStrictEqual(expectedCallsResult)
    expect(renderChunkCalls).toStrictEqual(expectedCallsResult)
    expect(buildEndCalls).toStrictEqual(expectedCallsResult)
    expect(transformCalls).toStrictEqual(expectedCallsResult)
    expect(moduleParsedCalls).toStrictEqual(expectedCallsResult)
    expect(loadCalls).toStrictEqual(expectedCallsResult)
    expect(augmentChunkHashCalls).toStrictEqual(expectedCallsResult)
    expect(renderStartCalls).toStrictEqual(expectedCallsResult)
    expect(generateBundleCalls).toStrictEqual(expectedCallsResult)
    expect(writeBundleCalls).toStrictEqual(expectedCallsResult)
    expect(bannerCalls).toStrictEqual(expectedCallsResult)
    expect(footerCalls).toStrictEqual(expectedCallsResult)
    expect(introCalls).toStrictEqual(expectedCallsResult)
    expect(outroCalls).toStrictEqual(expectedCallsResult)
  },
})
