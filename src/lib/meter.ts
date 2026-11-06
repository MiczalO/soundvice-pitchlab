export interface MeterSeg {
  on: boolean;
  hot: boolean;
  over: boolean;
}

/** Limit znaków pitchu w Spotify for Artists. */
export const PITCH_LIMIT = 500;

/**
 * Segmentowy miernik VU (port `meter()` z prototypu): 25 segmentów po 20 znaków;
 * od 440 znaków segmenty robią się bursztynowe, powyżej limitu — czerwone.
 */
export function meterSegments(n: number, segments = 25): MeterSeg[] {
  return Array.from({ length: segments }, (_, i) => {
    const lim = (i + 1) * 20;
    const on = n >= lim - 19;
    return { on, hot: on && lim > 440, over: on && n > PITCH_LIMIT };
  });
}

/** Liczenie znaków jak Spotify: ze spacjami, w znakach Unicode (nie bajtach, nie UTF-16). */
export const countChars = (s: string): number => [...s].length;
