import type {
  FilmChunkInput,
  FilmData,
} from '@gravllift/halo-helpers/src/film-data';
import type { FilmPlayerTeam } from './index';

export { FilmPlayerTeam };

export function parseFilm(chunks: FilmChunkInput[]): Promise<FilmData>;
export function parseFilmWithTeams(
  chunks: FilmChunkInput[],
  players: readonly FilmPlayerTeam[],
): Promise<FilmData>;
