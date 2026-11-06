import { describe, expect, it } from "vitest";
import { bestModel, modelLabel, type StatusMap } from "./Models";

const list = ["gemini-2.5-pro", "gemini-3.1-pro-preview", "gemini-3.8-flash", "gemini-3.6-flash"];

describe("bestModel", () => {
  it("skips retired and no-quota models, picks the first working one", () => {
    const st: StatusMap = {
      "gemini-2.5-pro": "retired",
      "gemini-3.1-pro-preview": "quota",
      "gemini-3.8-flash": "overloaded",
      "gemini-3.6-flash": "ok",
    };
    expect(bestModel(list, st)).toBe("gemini-3.6-flash");
  });
  it("falls back to an overloaded model, then to the first on the list", () => {
    expect(bestModel(list, { "gemini-3.8-flash": "overloaded" })).toBe("gemini-3.8-flash");
    expect(bestModel(list, {})).toBe("gemini-2.5-pro");
    expect(bestModel([], {})).toBeUndefined();
  });
});

describe("modelLabel", () => {
  it("adds a Polish status suffix only when status is known", () => {
    expect(modelLabel("a", { a: "quota" })).toBe("a — brak przydziału w Twoim planie");
    expect(modelLabel("b", {})).toBe("b");
  });
});
