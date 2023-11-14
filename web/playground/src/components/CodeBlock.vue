<script setup lang="ts">
import { ShallowRef, shallowRef } from 'vue'
import { Codemirror } from 'vue-codemirror'
import { javascript } from '@codemirror/lang-javascript'
import { oneDark } from '@codemirror/theme-one-dark'
import { EditorState } from '@codemirror/state'
import { EditorView } from '@codemirror/view'

type Payload = {
  state: EditorState
  view: EditorView
  container: HTMLElement
}

let props = defineProps({
  code: String,
  readonly: {
    type: Boolean,
    required: false,
  },
})

const extensions = [javascript(), oneDark]

if (props.readonly) {
  extensions.push(EditorView.editable.of(false))
}
// Codemirror EditorView instance ref
const view = <ShallowRef<EditorView>>shallowRef()
const handleReady = (payload: Payload) => {
  view.value = payload.view
}

// Status is available at all times via Codemirror EditorView
// const getCodemirrorStates = () => {
//   const state = view.value.state
//   const ranges = state.selection.ranges
//   const selected = ranges.reduce((r, range) => r + range.to - range.from, 0)
//   const cursor = ranges[0].anchor
//   const length = state.doc.length
//   const lines = state.doc.lines
//   return {
//     state,
//     ranges,
//     selected,
//     cursor,
//     length,
//     lines
//   }
// }
</script>
<template>
  <codemirror
    :model-value="code"
    placeholder="Code goes here..."
    :style="{ height: '400px' }"
    :autofocus="false"
    :indent-with-tab="true"
    :tab-size="2"
    :extensions="extensions"
    @ready="handleReady"
    @change="$emit('code', $event)"
  />
</template>
