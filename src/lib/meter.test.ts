import { describe, expect, it } from "vitest";
import { countChars, meterSegments } from "./meter";

const lit = (n: number) => meterSegments(n).filter((s) => s.on).length;

describe("meterSegments", () => {
  it("lights one segment per started 20 characters", () => {
    expect(lit(0)).toBe(0);
    expect(lit(1)).toBe(1);
    expect(lit(20)).toBe(1);
    expect(lit(21)).toBe(2);
    expect(lit(500)).toBe(25);
  });
  it("turns amber above 440 and red above the 500 limit", () => {
    expect(meterSegments(400).some((s) => s.hot)).toBe(false);
    expect(meterSegments(450).filter((s) => s.hot).length).toBe(1);
    expect(meterSegments(500).some((s) => s.over)).toBe(false);
    expect(
      meterSegments(501)
        .filter((s) => s.on)
        .every((s) => s.over),
    ).toBe(true);
  });
});

describe("countChars", () => {
  it("counts characters, not bytes or UTF-16 units", () => {
    expect(countChars("zażółć gęślą")).toBe(12);
    expect(countChars("a b")).toBe(3);
    expect(countChars("😀")).toBe(1);
  });
});
