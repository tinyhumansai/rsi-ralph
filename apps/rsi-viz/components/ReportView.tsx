/**
 * Renders a hive `experiment-report.json`: an arm × ablation table of trial
 * outcomes and a dot-and-whisker chart of each trial's anchor interval.
 */

import { cellKey, groupTrials, type ExperimentReport, type Trial } from "@/lib/report";
import { formatMetric, formatSigned, niceTicks } from "@/lib/format";

interface CountsProps {
  accepted: number;
  rejected: number;
  inconclusive: number;
  failed: number;
}

function Counts({ accepted, rejected, inconclusive, failed }: CountsProps) {
  return (
    <span className="counts">
      <span className="accept" title="accepted">
        ✓{accepted}
      </span>
      <span className="reject" title="rejected">
        ✕{rejected}
      </span>
      <span className="inconclusive" title="inconclusive">
        ◆{inconclusive}
      </span>
      <span className="muted" title="failed">
        !{failed}
      </span>
    </span>
  );
}

function AnchorChart({ rows }: { rows: { label: string; trial: Trial }[] }) {
  const width = 640;
  const rowH = 26;
  const left = 290;
  const right = 20;
  const top = 10;
  const height = top + rows.length * rowH + 34;
  const values = rows.flatMap(({ trial }) => [trial.anchor_lower ?? 0, trial.anchor_upper ?? 0, trial.anchor_gain ?? 0]);
  const ticks = niceTicks(Math.min(0, ...values), Math.max(0, ...values), 5);
  const lo = ticks[0] ?? -1;
  const hi = ticks[ticks.length - 1] ?? 1;
  const x = (v: number) => left + ((v - lo) / (hi - lo || 1)) * (width - left - right);
  const plotBottom = top + rows.length * rowH;
  return (
    <svg
      className="chart scaled"
      viewBox={`0 0 ${width} ${height}`}
      role="img"
      aria-label="Anchor gain intervals per trial"
      style={{ maxWidth: 760 }}
    >
      {ticks.map((t) => (
        <g key={t}>
          <line x1={x(t)} x2={x(t)} y1={top} y2={plotBottom} stroke="var(--grid)" />
          <text x={x(t)} y={plotBottom + 14} textAnchor="middle">
            {formatMetric(t)}
          </text>
        </g>
      ))}
      <line x1={x(0)} x2={x(0)} y1={top} y2={plotBottom} stroke="var(--axis)" strokeWidth={1.5} />
      <text x={(left + width - right) / 2} y={height - 4} textAnchor="middle">
        anchor gain (positive = improvement)
      </text>
      {rows.map(({ label, trial }, i) => {
        const y = top + i * rowH + rowH / 2;
        const color = trial.anchor_accepted ? "var(--accept)" : "var(--series)";
        const tip = `${label}: gain ${formatSigned(trial.anchor_gain)} [${formatMetric(trial.anchor_lower)}, ${formatMetric(
          trial.anchor_upper,
        )}]${trial.anchor_accepted ? ", accepted" : ""}`;
        return (
          <g key={`${label}-${i}`}>
            <title>{tip}</title>
            <rect x={0} y={y - rowH / 2} width={width} height={rowH} fill="transparent" />
            <text x={left - 8} y={y + 4} textAnchor="end">
              {label}
            </text>
            <line
              x1={x(trial.anchor_lower ?? 0)}
              x2={x(trial.anchor_upper ?? 0)}
              y1={y}
              y2={y}
              stroke={color}
              strokeWidth={2}
              strokeLinecap="round"
            />
            <circle cx={x(trial.anchor_gain ?? 0)} cy={y} r={4.5} fill={color} stroke="var(--surface)" strokeWidth={2} />
          </g>
        );
      })}
    </svg>
  );
}

export function ReportView({ report }: { report: ExperimentReport }) {
  const { arms, ablations, cells } = groupTrials(report.trials);
  const anchored = arms.flatMap((arm) =>
    ablations.flatMap((ablation) =>
      (cells.get(cellKey(arm, ablation))?.anchored ?? []).map((trial) => ({
        label: `${arm} · ${ablation} · ${trial.proposal_seed}`,
        trial,
      })),
    ),
  );
  const unanchored = report.trials.length - anchored.length;
  return (
    <div className="stack">
      <dl className="facts">
        {report.verdict !== undefined ? (
          <div>
            <dt>Verdict</dt>
            <dd>{report.verdict}</dd>
          </div>
        ) : null}
        {report.source !== undefined ? (
          <div>
            <dt>Source</dt>
            <dd>{report.source}</dd>
          </div>
        ) : null}
        <div>
          <dt>Trials</dt>
          <dd>{report.trials.length}</dd>
        </div>
        {report.generation_tokens !== undefined ? (
          <div>
            <dt>Generation tokens</dt>
            <dd>{report.generation_tokens.toLocaleString()}</dd>
          </div>
        ) : null}
        {report.evaluation_tokens !== undefined ? (
          <div>
            <dt>Evaluation tokens</dt>
            <dd>{report.evaluation_tokens.toLocaleString()}</dd>
          </div>
        ) : null}
      </dl>

      <div className="scroll-x">
        <table>
          <thead>
            <tr>
              <th>Arm</th>
              {ablations.map((ablation) => (
                <th key={ablation}>{ablation}</th>
              ))}
            </tr>
          </thead>
          <tbody>
            {arms.map((arm) => (
              <tr key={arm}>
                <th scope="row">{arm}</th>
                {ablations.map((ablation) => {
                  const cell = cells.get(cellKey(arm, ablation));
                  if (!cell) return <td key={ablation}>—</td>;
                  return (
                    <td key={ablation}>
                      <div className="secondary" style={{ fontSize: "0.8rem" }}>
                        {cell.trials} trials · {cell.anchorsAccepted} anchors · {(cell.tokens / 1000).toFixed(0)}k tok
                      </div>
                      <Counts
                        accepted={cell.accepted}
                        rejected={cell.rejected}
                        inconclusive={cell.inconclusive}
                        failed={cell.failed}
                      />
                    </td>
                  );
                })}
              </tr>
            ))}
          </tbody>
        </table>
        <p className="muted" style={{ fontSize: "0.8rem", margin: "6px 0 0" }}>
          Per cell: trials, anchors accepted, generation + evaluation tokens; then ✓ accepted, ✕ rejected, ◆
          inconclusive, ! failed proposal attempts.
        </p>
      </div>

      {report.ablations && report.ablations.length > 0 ? (
        <div className="scroll-x">
          <table>
            <thead>
              <tr>
                <th>Ablation</th>
                <th>Pairs</th>
                <th>Efficiency Δ</th>
                <th>Interval</th>
                <th>Anchors accepted</th>
              </tr>
            </thead>
            <tbody>
              {report.ablations.map((a) => (
                <tr key={a.ablation}>
                  <td>{a.ablation}</td>
                  <td>{a.pairs}</td>
                  <td>{formatSigned(a.efficiency_delta)}</td>
                  <td>
                    [{formatMetric(a.lower)}, {formatMetric(a.upper)}]
                  </td>
                  <td>{a.anchors_accepted ? "yes" : "no"}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      ) : null}

      <div>
        <h2>Anchor intervals</h2>
        {anchored.length > 0 ? (
          <AnchorChart rows={anchored} />
        ) : (
          <p className="notice">No trial produced an anchor interval.</p>
        )}
        <div className="legend">
          <span>
            <svg width="14" height="10" aria-hidden="true">
              <circle cx="7" cy="5" r="4" fill="var(--series)" />
            </svg>
            not accepted
          </span>
          <span>
            <svg width="14" height="10" aria-hidden="true">
              <circle cx="7" cy="5" r="4" fill="var(--accept)" />
            </svg>
            anchor accepted
          </span>
          {unanchored > 0 ? <span>{unanchored} trials without an anchor (failed or not verified) are omitted</span> : null}
        </div>
      </div>
    </div>
  );
}
