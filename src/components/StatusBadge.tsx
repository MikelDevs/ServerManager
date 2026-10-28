import type { ProcessStatus } from "../types/project";

const label: Record<ProcessStatus, string> = {
  running: "Activo",
  stopping: "Deteniendo",
  stopped: "Detenido",
};

export function StatusBadge({ status, compact = false }: { status: ProcessStatus; compact?: boolean }) {
  return (
    <span className={`status-badge status-${status}${compact ? " status-compact" : ""}`} title={label[status]} aria-label={compact ? label[status] : undefined}>
      <span className="status-dot" />
      {!compact && label[status]}
    </span>
  );
}

