import { describe, expect, it } from "vitest";
import { toAppError } from "./api";

describe("toAppError", () => {
  it("reads {code, message} returned by Rust", () => {
    expect(toAppError({ code: "bad_key", message: "Klucz odrzucony" })).toEqual({
      code: "bad_key",
      message: "Klucz odrzucony",
    });
  });
  it("falls back for plain strings", () => {
    expect(toAppError("boom")).toEqual({ code: "other", message: "boom" });
  });
});
