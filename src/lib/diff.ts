import { diffWordsWithSpace } from "diff";

export interface Seg {
  text: string;
  changed: boolean;
}

/**
 * Porównanie dwóch tekstów obok siebie: po lewej usunięte/zmienione fragmenty
 * oryginału, po prawej dodane/zmienione fragmenty propozycji.
 */
export function sideBySide(a: string, b: string): { left: Seg[]; right: Seg[] } {
  const left: Seg[] = [];
  const right: Seg[] = [];
  for (const part of diffWordsWithSpace(a, b)) {
    if (part.added) right.push({ text: part.value, changed: true });
    else if (part.removed) left.push({ text: part.value, changed: true });
    else {
      left.push({ text: part.value, changed: false });
      right.push({ text: part.value, changed: false });
    }
  }
  return { left, right };
}
