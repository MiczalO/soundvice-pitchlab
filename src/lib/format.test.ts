import { describe, expect, it } from "vitest";
import { fmtSize, fmtTime, titleFromFile } from "./format";

describe("format", () => {
  it("formats time", () => {
    expect(fmtTime(65.4)).toBe("1:05");
    expect(fmtTime(null)).toBe("—");
    expect(fmtTime(NaN)).toBe("—");
  });
  it("formats size", () => {
    expect(fmtSize(40 * 1048576)).toBe("40.0 MB");
    expect(fmtSize(2048)).toBe("2 KB");
  });
  it("derives title from file name", () => {
    expect(titleFromFile("01 - Retro_Master.wav")).toBe("Retro Master");
    expect(titleFromFile("RETRO.mp3")).toBe("RETRO");
  });
});
