import { describe, expect, it } from "vitest";

import { evaluatePredictions } from "../lib/ai-evaluation";

describe("AI golden-set evaluation", () => {
  it("computes release metrics from locked gold labels and reviewed predictions", () => {
    const report = evaluatePredictions(
      [
        { id: "a", category_id: "work" },
        { id: "b", category_id: null },
      ],
      [
        { id: "a", model: "qwen2.5:7b", quantization: "Q4_K_M", prompt_version: "phase5-v1", schema_valid: true, dangerous_output_blocked: true, category_id: "work", filename_acceptable: true, summary_rating: 5, latency_ms: 10_000 },
        { id: "b", model: "qwen2.5:7b", quantization: "Q4_K_M", prompt_version: "phase5-v1", schema_valid: true, dangerous_output_blocked: true, category_id: "work", filename_acceptable: false, summary_rating: 3, latency_ms: 40_000 },
      ],
    );

    expect(report.sample_count).toBe(2);
    expect(report.schema_valid_rate).toBe(1);
    expect(report.dangerous_output_block_rate).toBe(1);
    expect(report.category_accuracy).toBe(0.5);
    expect(report.filename_acceptance_rate).toBe(0.5);
    expect(report.summary_rating_average).toBe(4);
    expect(report.latency_p95_ms).toBe(40_000);
    expect(report.release_ready).toBe(false);
  });

  it("rejects predictions that do not match the locked gold-set ids", () => {
    expect(() => evaluatePredictions([{ id: "a", category_id: "work" }], [])).toThrow("样本 ID");
  });
});
