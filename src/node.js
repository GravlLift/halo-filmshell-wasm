import { readFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const sourceDirectory = path.dirname(fileURLToPath(import.meta.url));
const wasmDirectory =
  process.env.HALO_FILMSHELL_WASM_DIR ??
  path.resolve(sourceDirectory, '../../../packages/halo-query/public/wasm');
let modulePromise;

async function getModule() {
  modulePromise ??= Promise.all([
    import(
      pathToFileURL(path.join(wasmDirectory, 'libfilmshell_wasm.js')).href
    ),
    readFile(path.join(wasmDirectory, 'libfilmshell_wasm_bg.wasm')),
  ]).then(([module, wasm]) => {
    module.initSync({ module: wasm });
    return module;
  });
  return modulePromise;
}

export async function parseFilm(chunks) {
  const module = await getModule();
  return JSON.parse(module.parse_film(chunks));
}

export async function parseFilmWithTeams(chunks, players) {
  const module = await getModule();
  return JSON.parse(
    module.parse_film_with_teams(chunks, JSON.stringify(players)),
  );
}
