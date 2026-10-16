import type { ProcessStatus } from "../types/project";

const label: Record<ProcessStatus, string> = {
  running: "Running",
  stopping: "Stopping",
  stopped: "Stopped",
};

export function StatusBadge({ status, compact = false }: { status: ProcessStatus; compact?: boolean }) {
  return (
    <span className={`status-badge status-${status}${compact ? " status-compact" : ""}`}>
      <span className="status-dot" />
      {!compact && label[status]}
    </span>
  );
}

