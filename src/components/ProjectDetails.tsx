import { useEffect, useMemo, useState } from "react";
import type { ProcessOutputEvent, ProcessSnapshot, ProjectConfig } from "../types/project";
import { BrowserIcon, ClockIcon, CodeIcon, EditIcon, FolderIcon, PlayIcon, RestartIcon, StopIcon, TerminalIcon, TrashIcon } from "./Icons";
import { LogConsole } from "./LogConsole";
import { StatusBadge } from "./StatusBadge";

interface ProjectDetailsProps {
  project: ProjectConfig;
  snapshot?: ProcessSnapshot;
  logs: ProcessOutputEvent[];
  busy: boolean;
  onStart: () => Promise<unknown>;
  onStop: () => Promise<unknown>;
  onRestart: () => Promise<unknown>;
  onSendInput: (input: string) => Promise<unknown>;
  onClearLogs: () => void;
  onEdit: () => void;
  onDelete: () => void;
  onOpenInCode: () => Promise<unknown>;
  onOpenUrl: () => Promise<unknown>;
  onOpenTerminal: () => Promise<unknown>;
}

function formatUptime(startedAt: string | null | undefined, now: number): string {
  if (!startedAt) return "—";
  const seconds = Math.max(0, Math.floor((now - new Date(startedAt).getTime()) / 1000));
  const hours = Math.floor(seconds / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  const remainder = seconds % 60;
  return hours > 0
    ? `${hours}h ${minutes.toString().padStart(2, "0")}m ${remainder.toString().padStart(2, "0")}s`
    : `${minutes}m ${remainder.toString().padStart(2, "0")}s`;
}

export function ProjectDetails({
  project,
  snapshot,
  logs,
  busy,
  onStart,
  onStop,
  onRestart,
  onSendInput,
  onClearLogs,
  onEdit,
  onDelete,
  onOpenInCode,
  onOpenUrl,
  onOpenTerminal,
}: ProjectDetailsProps) {
  const [now, setNow] = useState(Date.now());
  const status = snapshot?.status ?? "stopped";

  useEffect(() => {
    if (status !== "running") return;
    const interval = window.setInterval(() => setNow(Date.now()), 1_000);
    return () => window.clearInterval(interval);
  }, [status]);

  const uptime = useMemo(() => formatUptime(snapshot?.startedAt, now), [snapshot?.startedAt, now]);

  return (
    <main className="main-panel">
      <header className="project-header">
        <div className="project-identity">
          <span className="project-hero-icon">{project.icon || "▰"}</span>
          <div>
            <div className="project-title-row">
              <h1>{project.name}</h1>
              <StatusBadge status={status} />
            </div>
            <div className="project-path" title={project.workingDirectory}>
              <FolderIcon /> {project.workingDirectory}
            </div>
          </div>
        </div>
        <div className="header-actions">
          <button className="button button-secondary button-small" onClick={() => void onOpenInCode()} title="Abrir carpeta en Visual Studio Code"><CodeIcon /> Code</button>
          <button className="button button-secondary button-small" onClick={() => void onOpenUrl()} title={project.projectUrl ? `Abrir ${project.projectUrl}` : "Configura una URL para habilitar esta acción"} disabled={!project.projectUrl}><BrowserIcon /> Abrir app</button>
          <button className="button button-secondary button-small" onClick={() => void onOpenTerminal()} title={project.sshIp ? `ssh root@${project.sshIp}` : "Configura la IP para SSH al editar este proyecto"} disabled={!project.sshIp || busy}><TerminalIcon /> Abrir terminal</button>
          <button className="icon-button" onClick={onEdit} title="Editar proyecto" aria-label="Editar proyecto"><EditIcon /></button>
          <button className="icon-button danger" onClick={onDelete} title="Eliminar proyecto" aria-label="Eliminar proyecto" disabled={status !== "stopped"}><TrashIcon /></button>
        </div>
      </header>

      <div className="control-strip">
        <div className="process-actions">
          <button className="button button-success" onClick={() => void onStart()} disabled={status !== "stopped" || busy}>
            <PlayIcon /> Start
          </button>
          <button className="button button-secondary" onClick={() => void onStop()} disabled={status === "stopped" || busy}>
            <StopIcon /> Stop
          </button>
          <button className="button button-secondary" onClick={() => void onRestart()} disabled={status === "stopped" || busy}>
            <RestartIcon /> Restart
          </button>
        </div>
        <div className="runtime-stats">
          <div>
            <span>PID</span>
            <strong>{snapshot?.pid ?? "—"}</strong>
          </div>
          <div>
            <span><ClockIcon /> TIEMPO ACTIVO</span>
            <strong>{status === "running" ? uptime : "—"}</strong>
          </div>
        </div>
      </div>

      <section className="config-summary">
        <div>
          <span>COMANDO DE INICIO</span>
          <code>{project.startCommand}</code>
        </div>
        <div>
          <span>PARADA POR STDIN</span>
          <code>{project.stopCommand || "Terminación del sistema"}</code>
        </div>
        <div>
          <span>ENTORNO</span>
          <strong>{Object.keys(project.environmentVariables).length} variables</strong>
        </div>
        {project.projectUrl && (
          <div className="project-url-summary">
            <span>URL</span>
            <code>{project.projectUrl}</code>
          </div>
        )}
      </section>

      <LogConsole
        projectId={project.id}
        status={status}
        logs={logs}
        onSendInput={onSendInput}
        onClear={onClearLogs}
      />
    </main>
  );
}
