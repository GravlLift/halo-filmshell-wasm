let modulePromise;

export async function parseFilm(chunks) {
  modulePromise ??= import(
    /* webpackIgnore: true */ '/wasm/libfilmshell_wasm.js'
  ).then(async (module) => {
    await module.default();
    return module;
  });
  const module = await modulePromise;
  return JSON.parse(module.parse_film(chunks));
}
