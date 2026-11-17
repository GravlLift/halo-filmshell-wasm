let modulePromise;

async function getModule() {
  modulePromise ??= import(
    /* webpackIgnore: true */ '/wasm/libfilmshell_wasm.js'
  ).then(async (module) => {
    await module.default();
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
