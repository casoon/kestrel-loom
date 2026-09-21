/**
 * Puts the built chart next to the demo page.
 *
 * `public/demo/start.js` belongs to the site and is committed. The wrapper
 * (`js/kestrel-loom.js`) and the WASM package (`pkg/`, from `./build-wasm.sh`)
 * are copied here on every build instead — committed copies would sooner or
 * later show a state nobody builds any more. The layout `lib/js` + `lib/pkg`
 * keeps the wrapper's own import `../pkg/kestrel_loom_wasm.js` intact.
 */

import { copyFile, mkdir, stat } from 'node:fs/promises';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const repo = dirname(dirname(dirname(fileURLToPath(import.meta.url))));
const target = join(repo, 'site', 'public', 'demo', 'lib');

const files = [
  ['js/kestrel-loom.js', 'js/kestrel-loom.js'],
  ['pkg/kestrel_loom_wasm.js', 'pkg/kestrel_loom_wasm.js'],
  ['pkg/kestrel_loom_wasm_bg.wasm', 'pkg/kestrel_loom_wasm_bg.wasm'],
];

for (const [from] of files) {
  try {
    await stat(join(repo, from));
  } catch {
    console.error(`${from} is missing — run ./build-wasm.sh in the repository root first.`);
    process.exit(1);
  }
}

for (const [from, to] of files) {
  await mkdir(dirname(join(target, to)), { recursive: true });
  await copyFile(join(repo, from), join(target, to));
}

console.log(`js/ + pkg/ → site/public/demo/lib/ (${files.length} files)`);
