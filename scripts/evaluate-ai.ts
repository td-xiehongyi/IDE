import { readFileSync } from "node:fs";

import {
  evaluatePredictions,
  type GoldSample,
  type ReviewedPrediction,
} from "../src/lib/ai-evaluation.ts";

function parseJsonLines<T>(path: string): T[] {
  return readFileSync(path, "utf8")
    .split(/\r?\n/)
    .map((line: string) => line.trim())
    .filter(Boolean)
    .map((line: string, index: number) => {
      try {
        return JSON.parse(line) as T;
      } catch (error) {
        throw new Error(`${path} 第 ${index + 1} 行不是有效 JSON：${String(error)}`);
      }
    });
}

const [, , goldPath, predictionsPath] = process.argv;
if (!goldPath || !predictionsPath) {
  console.error("用法：pnpm evaluate:ai <gold.jsonl> <reviewed-predictions.jsonl>");
  process.exitCode = 2;
} else {
  try {
    const report = evaluatePredictions(
      parseJsonLines<GoldSample>(goldPath),
      parseJsonLines<ReviewedPrediction>(predictionsPath),
    );
    console.log(JSON.stringify(report, null, 2));
    process.exitCode = report.release_ready ? 0 : 1;
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    process.exitCode = 2;
  }
}
