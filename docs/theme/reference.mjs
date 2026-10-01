// The API reference's sections and sidebar, from its pages
// (`reference/pages.json`, generated with the reference) and the starred
// pages and category order of `navigation.json`.
import referencePages from '#theme/reference-pages';

import navigation from '../navigation.json' with { type: 'json' };

const { starred, categories: order } = navigation.reference;

const item = ({ name, url }) => ({
  text: name,
  link: url,
  starred: starred.includes(url.split('/').at(-1)),
});

const rank = (category) => (order.includes(category) ? order.indexOf(category) : order.length);

// The types whose members have pages are listed through them
const owners = new Set(referencePages.map(({ owner }) => owner));

const categories = Map.groupBy(
  referencePages.filter(({ name, owner, inlined }) => !owner && !inlined && !owners.has(name)),
  ({ category }) => category ?? 'Other',
);

const options = (owner) => referencePages.filter((page) => page.owner === owner).map(item);

/**
 * The reference's sections: the input and output options, then the exports
 * by category, starred ones first.
 */
export const referenceSections = [
  { text: 'Input Options', items: options('InputOptions') },
  { text: 'Output Options', items: options('OutputOptions') },
  ...[...categories]
    .sort(([a], [b]) => rank(a) - rank(b))
    .map(([text, pages]) => ({
      text,
      items: pages
        .map(item)
        .sort((a, b) => Number(b.starred) - Number(a.starred) || a.text.localeCompare(b.text)),
    })),
];

const [inputOptions, outputOptions, ...apis] = referenceSections;

/**
 * The reference's sidebar: the options, with the output options folded under
 * `output`, then the exports by category, starred ones marked.
 */
export const referenceSidebar = [
  {
    text: 'Options',
    collapsed: false,
    items: [...inputOptions.items, { text: 'output', collapsed: true, items: outputOptions.items }],
  },
  ...apis.map(({ text, items }) => ({
    text,
    collapsed: true,
    items: items.map(({ text, link, starred }) => ({ text: starred ? `★ ${text}` : text, link })),
  })),
];
