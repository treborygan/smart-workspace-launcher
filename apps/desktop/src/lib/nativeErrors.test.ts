import { describe, expect, it } from "vitest";
import { errorMessage, withNativeErrorContext } from "./nativeErrors";

describe("native command errors", () => {
  it("keeps useful string rejections instead of replacing them with a generic message", () => {
    expect(errorMessage("profile not found", "Unknown error")).toBe("profile not found");
  });

  it("uses Error messages and falls back for empty or non-text failures", () => {
    expect(errorMessage(new Error("database unavailable"), "Unknown error")).toBe("database unavailable");
    expect(errorMessage("  ", "Unknown error")).toBe("Unknown error");
    expect(errorMessage({ code: 5 }, "Unknown error")).toBe("Unknown error");
  });

  it("adds the failing native startup operation to a string rejection", async () => {
    await expect(withNativeErrorContext("Loading resources", async () => {
      throw "workspace unavailable";
    })).rejects.toThrow("Loading resources: workspace unavailable");
  });
});
