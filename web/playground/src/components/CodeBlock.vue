<script setup lang="ts">
import { ShallowRef, defineComponent, ref, shallowRef } from 'vue'
import { Codemirror } from 'vue-codemirror'
import { javascript } from '@codemirror/lang-javascript'
import { oneDark } from '@codemirror/theme-one-dark'
import { EditorState } from '@codemirror/state'
import { EditorView } from '@codemirror/view'

type Payload = {
  state: EditorState,
  view: EditorView,
  container: HTMLElement
}

defineProps({
  code: String
})

const emit = defineEmits(['code'])

const extensions = [javascript(), oneDark]

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

const handleCodeChange = (e: string) => {
  emit("code", e)
}
const log = console.log;
</script>
<template>
  <codemirror :model-value="code"  placeholder="Code goes here..." :style="{ height: '400px' }" :autofocus="true"
    :indent-with-tab="true" :tab-size="2" :extensions="extensions" @ready="handleReady" @change="handleCodeChange"
    @focus="log('focus', $event)" @blur="console.log('blur', $event)" />
</template>


