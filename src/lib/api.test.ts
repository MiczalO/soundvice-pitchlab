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

import { withTimeout } from "./api";

describe("withTimeout", () => {
  it("rejects with a timeout error when the promise never settles", async () => {
    await expect(withTimeout(new Promise(() => {}), 20)).rejects.toMatchObject({ code: "timeout" });
  });
  it("passes through resolved values", async () => {
    await expect(withTimeout(Promise.resolve(7), 1000)).resolves.toBe(7);
  });
});
