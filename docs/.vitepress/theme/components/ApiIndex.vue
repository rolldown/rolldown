<!-- Reference: https://github.com/vuejs/docs/blob/main/src/api/ApiIndex.vue -->
<script setup lang="ts">
// in .vue components or .md pages:
// named import "data" is the resolved static data
// can also import types for type consistency
import { data as apiIndex } from './api.data';
import { ref, computed, onMounted } from 'vue';

const search = ref<HTMLInputElement>();
const query = ref('');

// Normalize: lowercase, replace hyphens with spaces, collapse whitespace, trim
const normalize = (s: string) => s.toLowerCase().replace(/-/g, ' ').replace(/\s+/g, ' ').trim();

// Split camelCase into words for matching, e.g. "codeSplitting" → ["code", "splitting"]
const splitCamelCase = (s: string): string[] =>
  s
    .replace(/([a-z])([A-Z])/g, '$1 $2')
    .replace(/([A-Z]+)([A-Z][a-z])/g, '$1 $2')
    .toLowerCase()
    .split(/\s+/);

// Score match quality for ranking results
const scoreMatch = (text: string, query: string): number => {
  const normalizedText = normalize(text);
  const normalizedQuery = normalize(query);

  if (!normalizedQuery) return 100; // Empty query shows all

  // Exact match (highest priority)
  if (normalizedText === normalizedQuery) return 100;

  // Prefix match
  if (normalizedText.startsWith(normalizedQuery)) return 80;

  // Word boundary match (camelCase aware)
  const words = splitCamelCase(text);
  if (words.some((w) => w.startsWith(normalizedQuery))) return 60;

  // Contains match
  if (normalizedText.includes(normalizedQuery)) return 40;

  // Multi-word query: all words must match
  const queryWords = normalizedQuery.split(/\s+/).filter(Boolean);
  if (queryWords.length > 1 && queryWords.every((qw) => normalizedText.includes(qw))) return 20;

  return 0; // No match
};

onMounted(() => {
  search.value?.focus();
});

const filtered = computed(() => {
  const q = query.value;

  return apiIndex
    .map((section) => {
      const sectionScore = scoreMatch(section.text, q);

      // Section title match - include all items with consistent score structure
      if (sectionScore > 0) {
        return {
          ...section,
          items: section.items.map((item) => ({ ...item, score: 100 })),
          score: sectionScore,
        };
      }

      // Filter and score individual items
      const scoredItems = section.items
        .map((item) => ({ ...item, score: scoreMatch(item.text, q) }))
        .filter((item) => item.score > 0)
        .sort((a, b) => b.score - a.score);

      return scoredItems.length
        ? {
            text: section.text,
            items: scoredItems,
            score: scoredItems[0].score,
          }
        : null;
    })
    .filter((i): i is NonNullable<typeof i> => !!i)
    .sort((a, b) => b.score - a.score);
});
</script>

<template>
  <div id="api-index">
    <div class="header">
      <h1>Options & APIs Reference</h1>
      <div class="api-filter">
        <label for="api-filter">Filter</label>
        <input
          ref="search"
          type="search"
          placeholder="Enter keyword"
          id="api-filter"
          v-model="query"
        />
      </div>
    </div>

    <p>
      These are the automatically generated references for Rolldown's options and APIs. Use the
      sidebar navigation to browse specific options and APIs.
    </p>

    <div v-for="section of filtered" :key="section.text" class="api-section">
      <h2>{{ section.text }}</h2>
      <div class="api-references">
        <a v-for="item of section.items" :key="item.text" :href="item.link" class="api-reference">
          <div>{{ item.text }}</div>
        </a>
      </div>
    </div>

    <div v-if="!filtered.length" class="no-match">
      No API reference matching "{{ query }}" found.
    </div>
  </div>
</template>

<style scoped>
#api-index {
  max-width: 1024px;
  margin: 0px auto;
}

h1,
h2 {
  font-weight: 600;
  line-height: 1;
}

h1,
h2 {
  letter-spacing: -0.02em;
}

h1 {
  font-size: 38px;
}

h2 {
  font-size: 24px;
  color: var(--vt-c-text-1);
  transition: color 0.5s;
  padding-top: 36px;
  border-top: 1px solid;
}

.api-section {
  margin-bottom: 64px;
}

.api-references {
  display: grid;
  gap: 14px;
}

.api-reference {
  break-inside: avoid;
  display: flex;
  flex-direction: column;
  background-color: var(--vt-c-bg-soft);
  border: 1px solid rgba(128, 128, 128, 0.2);
  border-radius: 8px;
  padding: 24px 12px;
  text-decoration: none;
  transition: border-color 0.25s;
  width: 100%;
  box-sizing: border-box;
  font-size: 14px;
  color: var(--vt-c-text-1);
}

.api-reference:hover {
  border-color: var(--vt-c-brand);
}

.header {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.api-filter {
  display: flex;
  align-items: center;
  justify-content: flex-start;
  gap: 1rem;
}

#api-filter {
  border: 1px solid var(--vt-c-divider);
  border-radius: 8px;
  padding: 6px 12px;
  transition: box-shadow 0.25s ease;
}

#api-filter:focus {
  box-shadow: 0 0 4pt var(--vt-c-brand);
}

.api-filter:focus {
  border-color: var(--vt-c-green-light);
}

.no-match {
  font-size: 1.2em;
  color: var(--vt-c-text-3);
  text-align: center;
  margin-top: 36px;
  padding-top: 36px;
  border-top: 1px solid var(--vt-c-divider-light);
}

@media (max-width: 768px) {
  #api-index {
    padding: 42px 24px;
  }

  h1 {
    font-size: 32px;
    margin-bottom: 24px;
  }

  h2 {
    font-size: 22px;
    margin: 42px 0 32px;
    padding-top: 32px;
  }

  .api-references a {
    font-size: 14px;
  }

  .header {
    display: block;
  }
}

@media (min-width: 768px) {
  .api-references {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }
}

@media (min-width: 1024px) {
  .api-references {
    grid-template-columns: repeat(3, minmax(0, 1fr));
  }
}
</style>
