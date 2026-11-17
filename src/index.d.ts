import type { FilmChunkInput, FilmData } from '../../halo-film/src/film-data';

export interface FilmPlayerTeam {
  xuid: string;
  team: number;
}

export type {
  FilmChunkInput,
  FilmClockData,
  FilmData,
  FilmLevelBoxData,
  FilmLevelData,
  FilmParser,
  FilmPlayerStateData,
  JsonValue,
} from '../../halo-film/src/film-data';

export function parseFilm(chunks: FilmChunkInput[]): Promise<FilmData>;
export function parseFilmWithTeams(
  chunks: FilmChunkInput[],
  players: readonly FilmPlayerTeam[],
): Promise<FilmData>;
