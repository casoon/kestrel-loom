// @ts-check
import casoonPages from '@casoon/pages-theme';
import { defineConfig } from 'astro/config';

// Project page: https://casoon.github.io/kestrel-loom/ — `base` is the GitHub Pages path.
export default defineConfig({
  site: 'https://casoon.github.io/kestrel-loom',
  base: '/kestrel-loom/',
  integrations: [
    casoonPages({
      name: 'kestrel-loom',
      description:
        'Interactive trading-chart renderer: Rust → WebAssembly → Canvas 2D, with the indicator math of kestrel-chartkit.',
      repo: 'casoon/kestrel-loom',
      version: '0.1.0',
      license: 'BUSL-1.1',
      // Not published to crates.io or npm: there is no registry or docs.rs page to link.
      packages: [],
      // The chart only shows what it is once it moves, so a live demo takes the
      // place a build-time showcase holds for other projects.
      demo: 'Demo',
      mdxComponents: './src/mdx-components.ts',
      showcase: false,
      // No CHANGELOG.md and no release notes yet.
      changelog: false,
      docsGroups: {
        'getting-started': 'Getting started',
        guides: 'Guides',
        concepts: 'Concepts',
        reference: 'Reference',
      },
    }),
  ],
});
