import { describe, expect, it } from "vitest";
import { formatDuration, formatMetric, formatSigned, niceTicks, shortId } from "./format";

describe("format", () => {
  it("formats metrics and missing values", () => {
    expect(formatMetric(0.123456)).toBe("0.1235");
    expect(formatMetric(0)).toBe("0");
    expect(formatMetric(null)).toBe("—");
    expect(formatMetric(Number.NaN)).toBe("—");
    expect(formatMetric(0.00001)).toBe("1.00e-5");
    expect(formatMetric(123456)).toBe("1.23e+5");
    expect(formatSigned(0.31)).toBe("+0.31");
    expect(formatSigned(-0.08)).toBe("-0.08");
    expect(formatSigned(undefined)).toBe("—");
  });

  it("formats durations", () => {
    expect(formatDuration(850)).toBe("850 ms");
    expect(formatDuration(12_340)).toBe("12.3 s");
    expect(formatDuration(245_000)).toBe("4m 05s");
    expect(formatDuration(3_900_000)).toBe("1h 05m");
  });

  it("shortens ids", () => {
    expect(shortId("a1b2c3d4e5f6")).toBe("a1b2c3d4");
    expect(shortId("p1")).toBe("p1");
  });

  it("produces nice ticks that cover the range", () => {
    expect(niceTicks(0.04, 0.42)).toEqual([0, 0.1, 0.2, 0.3, 0.4, 0.5]);
    expect(niceTicks(0, 10, 5)).toEqual([0, 2, 4, 6, 8, 10]);
    const flat = niceTicks(1, 1);
    expect(flat[0]).toBeLessThanOrEqual(1);
    expect(flat[flat.length - 1]).toBeGreaterThanOrEqual(1);
    expect(niceTicks(5, -5).length).toBeGreaterThan(2);
    expect(niceTicks(Number.NaN, 1)).toEqual([]);
  });
});
