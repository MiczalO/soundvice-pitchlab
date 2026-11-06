import { describe, expect, it } from "vitest";
import { sideBySide } from "./diff";

const join = (segs: { text: string }[]) => segs.map((s) => s.text).join("");

describe("sideBySide", () => {
  it("keeps both sides intact and marks only changed words", () => {
    const a = "Mata jest raperem z Warszawy.";
    const b = "Mata jest raperem z Warszawy, debiutował w 2018.";
    const { left, right } = sideBySide(a, b);
    expect(join(left)).toBe(a);
    expect(join(right)).toBe(b);
    expect(right.some((s) => s.changed && s.text.includes("2018"))).toBe(true);
    expect(left.every((s) => !s.changed || s.text.trim() !== "")).toBe(true);
  });

  it("flags removed words on the left only", () => {
    const { left, right } = sideBySide("a b c", "a c");
    expect(left.filter((s) => s.changed).map((s) => s.text.trim())).toContain("b");
    expect(right.some((s) => s.changed)).toBe(false);
  });

  it("identical texts have no highlighted segments", () => {
    const { left, right } = sideBySide("to samo", "to samo");
    expect([...left, ...right].some((s) => s.changed)).toBe(false);
  });
});
