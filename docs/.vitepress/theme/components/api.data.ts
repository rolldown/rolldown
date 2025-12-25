import fs from 'node:fs';
import { fileURLToPath } from 'node:url';

export interface APIReference {
  text: string;
  anchor: string;
  items: {
    text: string;
    link: string;
  }[];
}

export interface TypedocSidebarItem {
  text: string;
  link: string;
}

export interface TypedocSidebarGroup {
  text: string;
  collapsed: boolean;
  items: TypedocSidebarItem[];
}

export type TypedocSidebar = TypedocSidebarGroup[];

// Declare the resolved data type for API groups
export declare const data: APIReference[];

// Utility function to generate a slug from a string (used for anchor links)
function slugify(text: string): string {
  return (
    text
      // Replace special characters and spaces with hyphens
      .replace(/[\s~`!@#$%^&*()\-_+=[\]{}|\\;:"'<>,.?/]+/g, '-')
      // Remove continuous separators
      .replace(/-{2,}/g, '-')
      // Remove leading/trailing hyphens
      .replace(/^-+|-+$/g, '')
      // Ensure it doesn't start with a number (e.g. #121)
      .replace(/^(\d)/, '_$1')
      // Convert to lowercase
      .toLowerCase()
  );
}

// Utility function to transform a link by removing .md extension and adding /reference prefix
function transformLink(link: string): string {
  return '/reference' + link.replace('.md', '');
}

// Main export function for loading the API data
export default {
  // Load API data and process sidebar items
  load(): APIReference[] {
    const inputOptions: TypedocSidebarItem[] = [];
    const outputOptions: TypedocSidebarItem[] = [];

    const optionSidebarPath = fileURLToPath(
      new URL('../../../reference/options-sidebar.json', import.meta.url),
    );

    const optionSidebar: TypedocSidebar = JSON.parse(
      fs.readFileSync(optionSidebarPath, 'utf-8'),
    );

    optionSidebar.forEach((item) => {
      if (item.text === 'output' && 'items' in item) {
        outputOptions.push(...item.items);
      } else if ('link' in item) {
        inputOptions.push(item as TypedocSidebarItem);
      }
    });

    const transformedOptionSidebar: APIReference[] = [
      {
        text: 'Input Options',
        anchor: slugify('Input Options'),
        items: inputOptions.map((item) => ({
          ...item,
          link: transformLink(item.link),
        })),
      },
      {
        text: 'Output Options',
        anchor: slugify('Output Options'),
        items: outputOptions.map((item) => ({
          ...item,
          link: transformLink(item.link),
        })),
      },
    ];

    const apiSidebarPath = fileURLToPath(
      new URL('../../../reference/typedoc-sidebar.json', import.meta.url),
    );

    const apiSidebar: TypedocSidebar = JSON.parse(
      fs.readFileSync(apiSidebarPath, 'utf-8'),
    );

    const transformedApiSidebar: APIReference[] = apiSidebar.map((group) => ({
      text: group.text,
      anchor: slugify(group.text),
      items: group.items.map((item) => ({
        ...item,
        link: transformLink(item.link),
      })),
    }));

    return [...transformedOptionSidebar, ...transformedApiSidebar];
  },
};
