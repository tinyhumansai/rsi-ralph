/**
 * Shape and summaries of the hive `experiment-report.json` snapshot.
 *
 * Only the fields the visualizer renders are typed; the full file is still
 * available through the raw JSON view.
 */

export interface Trial {
  arm: string;
  ablation: string;
  proposal_seed: number;
  attempts: number;
  accepted: number;
  rejected: number;
  inconclusive: number;
  failed: number;
  generation_tokens: number;
  evaluation_tokens: number;
  verified: boolean;
  failure: string | null;
  anchor_gain: number | null;
  anchor_lower: number | null;
  anchor_upper: number | null;
  anchor_accepted: boolean;
}

export interface AblationSummary {
  ablation: string;
  pairs: number;
  efficiency_delta: number;
  lower: number;
  upper: number;
  anchors_accepted: boolean;
}

export interface ExperimentReport {
  source?: string;
  verdict?: string;
  attempts?: number;
  generation_tokens?: number;
  evaluation_tokens?: number;
  trials: Trial[];
  ablations?: AblationSummary[];
}

/** Aggregate of every trial sharing one arm and ablation. */
export interface Cell {
  arm: string;
  ablation: string;
  trials: number;
  accepted: number;
  rejected: number;
  inconclusive: number;
  failed: number;
  tokens: number;
  anchorsAccepted: number;
  /** Trials that produced an anchor interval (`anchor_gain` not null). */
  anchored: Trial[];
}

/** Narrows unknown JSON to an experiment report, or returns null. */
export function asExperimentReport(value: unknown): ExperimentReport | null {
  if (typeof value !== "object" || value === null) return null;
  const trials = (value as { trials?: unknown }).trials;
  if (!Array.isArray(trials)) return null;
  const valid = trials.every(
    (t) => typeof t === "object" && t !== null && typeof t.arm === "string" && typeof t.ablation === "string",
  );
  return valid ? (value as ExperimentReport) : null;
}

/** Groups trials into arm × ablation cells, preserving first-seen order. */
export function groupTrials(trials: Trial[]): { arms: string[]; ablations: string[]; cells: Map<string, Cell> } {
  const arms: string[] = [];
  const ablations: string[] = [];
  const cells = new Map<string, Cell>();
  for (const t of trials) {
    if (!arms.includes(t.arm)) arms.push(t.arm);
    if (!ablations.includes(t.ablation)) ablations.push(t.ablation);
    const key = cellKey(t.arm, t.ablation);
    const cell = cells.get(key) ?? {
      arm: t.arm,
      ablation: t.ablation,
      trials: 0,
      accepted: 0,
      rejected: 0,
      inconclusive: 0,
      failed: 0,
      tokens: 0,
      anchorsAccepted: 0,
      anchored: [],
    };
    cell.trials += 1;
    cell.accepted += t.accepted;
    cell.rejected += t.rejected;
    cell.inconclusive += t.inconclusive;
    cell.failed += t.failed;
    cell.tokens += t.generation_tokens + t.evaluation_tokens;
    if (t.anchor_accepted) cell.anchorsAccepted += 1;
    if (t.anchor_gain !== null) cell.anchored.push(t);
    cells.set(key, cell);
  }
  return { arms, ablations, cells };
}

export function cellKey(arm: string, ablation: string): string {
  return `${arm}\u0000${ablation}`;
}
