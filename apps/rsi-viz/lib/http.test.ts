import { describe, expect, it } from "vitest";
import { errorResponse, routeParam } from "./http";
import { PathRejected } from "./paths";
import { SnapshotTooLarge } from "./snapshot";

describe("errorResponse", () => {
  it("maps rejected, missing, and other failures", async () => {
    expect(errorResponse(new PathRejected("nope")).status).toBe(400);
    expect(await errorResponse(new PathRejected("nope")).json()).toEqual({ error: "nope" });
    expect(errorResponse(Object.assign(new Error("x"), { code: "ENOENT" })).status).toBe(404);
    expect(errorResponse(Object.assign(new Error("x"), { code: "ENOTDIR" })).status).toBe(404);
    expect(errorResponse(new SnapshotTooLarge(1 << 30)).status).toBe(413);
    expect(errorResponse(new Error("boom")).status).toBe(500);
    expect(errorResponse(null).status).toBe(500);
  });
});

describe("routeParam", () => {
  it("decodes an encoded run id once and tolerates bad escapes", () => {
    expect(routeParam("rsi-script-loop%2Frun-1")).toBe("rsi-script-loop/run-1");
    expect(routeParam("plain")).toBe("plain");
    expect(routeParam("bad%zz")).toBe("bad%zz");
  });
});
