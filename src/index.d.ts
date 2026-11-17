import type { FilmChunkInput, FilmData } from '../../halo-film/src/film-data';

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
