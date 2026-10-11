import { readFileSync } from "node:fs";
import path from "node:path";
import { describe, expect, it } from "vitest";
import { parseEnvelope, stageOf } from "./events";

const fixture = readFileSync(path.join(__dirname, "../fixtures/curve-fit/events.jsonl"), "utf8")
  .split("\n")
  .filter(Boolean);

describe("parseEnvelope", () => {
  it("accepts every line of the fixture trace", () => {
    for (const line of fixture) expect(parseEnvelope(line).ok, line).toBe(true);
  });

  it("accepts null for optional metrics", () => {
    const r = parseEnvelope(
      '{"seq":1,"elapsed_ms":2,"event":{"kind":"executed","epoch":0,"candidate":"x","seed":1,"metric":null,"ok":false,"duration_ms":3}}',
    );
    expect(r.ok).toBe(true);
  });

  it("accepts an ideated proposal with a null candidate and rejects an omitted one", () => {
    const base = '{"seq":0,"elapsed_ms":0,"event":{"kind":"ideated","epoch":1,"proposal_id":"p","parent":"a"';
    expect(parseEnvelope(`${base},"candidate":null,"rationale":"r","hypothesis":"h"}}`).ok).toBe(true);
    expect(parseEnvelope(`${base},"candidate":"c","rationale":"r","hypothesis":"h"}}`).ok).toBe(true);
    expect(parseEnvelope(`${base},"rationale":"r","hypothesis":"h"}}`)).toEqual({
      ok: false,
      error: "ideated.candidate is not string|null",
    });
    expect(parseEnvelope(`${base},"candidate":7,"rationale":"r","hypothesis":"h"}}`).ok).toBe(false);
  });

  it("accepts an optional approach label on ideated and rejects a mistyped one", () => {
    const base =
      '{"seq":0,"elapsed_ms":0,"event":{"kind":"ideated","epoch":1,"proposal_id":"p","parent":"a","candidate":"c","rationale":"r","hypothesis":"h"';
    expect(parseEnvelope(`${base}}}`).ok).toBe(true);
    expect(parseEnvelope(`${base},"approach":null}}`).ok).toBe(true);
    expect(parseEnvelope(`${base},"approach":"tabu search"}}`).ok).toBe(true);
    expect(parseEnvelope(`${base},"approach":3}}`)).toEqual({
      ok: false,
      error: "ideated.approach is not string|null",
    });
  });

  it("rejects invalid json", () => {
    expect(parseEnvelope("{not json")).toEqual({ ok: false, error: "invalid json" });
  });

  it("rejects an unknown kind", () => {
    const r = parseEnvelope('{"seq":0,"elapsed_ms":0,"event":{"kind":"teleported"}}');
    expect(r).toEqual({ ok: false, error: "unknown event kind teleported" });
  });

  it("rejects an omitted optional number", () => {
    const r = parseEnvelope('{"seq":0,"elapsed_ms":0,"event":{"kind":"epoch_finished","epoch":1,"incumbent":"x"}}');
    expect(r.ok).toBe(false);
  });

  it("rejects a missing envelope field", () => {
    expect(parseEnvelope('{"elapsed_ms":0,"event":{"kind":"run_started"}}').ok).toBe(false);
    expect(parseEnvelope("[1]").ok).toBe(false);
    expect(parseEnvelope('{"seq":0,"elapsed_ms":0}').ok).toBe(false);
  });

  it("reads a scored interval's units, defaulting to the metric", () => {
    const line = (units: string) =>
      `{"seq":0,"elapsed_ms":0,"event":{"kind":"scored","epoch":1,"candidate":"x","decision":"accept","estimate":0.5,"lower":0.25,"upper":0.75${units}}}`;
    const quality = parseEnvelope(line(',"units":"quality"'));
    expect(quality.ok && quality.envelope.event.kind === "scored" && quality.envelope.event.units).toBe("quality");
    const metric = parseEnvelope(line(""));
    expect(metric.ok && metric.envelope.event.kind === "scored" && metric.envelope.event.units).toBe(undefined);
    expect(parseEnvelope(line(',"units":"furlongs"')).ok).toBe(false);
  });

  it("rejects an unknown decision, finish reason, or direction", () => {
    expect(
      parseEnvelope(
        '{"seq":0,"elapsed_ms":0,"event":{"kind":"scored","epoch":1,"candidate":"x","decision":"maybe","estimate":0,"lower":0,"upper":0}}',
      ).ok,
    ).toBe(false);
    expect(
      parseEnvelope(
        '{"seq":0,"elapsed_ms":0,"event":{"kind":"run_finished","reason":"bored","incumbent":"x","incumbent_metric":null}}',
      ).ok,
    ).toBe(false);
    expect(
      parseEnvelope(
        '{"seq":0,"elapsed_ms":0,"event":{"kind":"run_started","run_id":"r","subject":"s","goal":{"metric":"m","direction":"up","target":null}}}',
      ).ok,
    ).toBe(false);
  });
});

describe("stageOf", () => {
  it("maps loop events to ring stages and bookkeeping events to none", () => {
    expect(stageOf("observed")).toBe("observe");
    expect(stageOf("ideated")).toBe("ideate");
    expect(stageOf("executed")).toBe("execute");
    expect(stageOf("measured")).toBe("measure");
    expect(stageOf("scored")).toBe("score");
    expect(stageOf("improved")).toBe("improve");
    expect(stageOf("run_started")).toBeNull();
    expect(stageOf("epoch_finished")).toBeNull();
  });
});
