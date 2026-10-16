import type { ProcessSnapshot, ProjectConfig } from "../types/project";
import { PlayIcon, PlusIcon, StopIcon } from "./Icons";
import { StatusBadge } from "./StatusBadge";

interface ProjectSidebarProps {
  projects: ProjectConfig[];
  snapshots: Record<string, ProcessSnapshot>;
  selectedId: string | null;
  onSelect: (projectId: string) => void;
  onCreate: () => void;
  onStartAll: () => void;
  onStopAll: () => void;
}

export function ProjectSidebar({
  projects,
  snapshots,
  selectedId,
  onSelect,
  onCreate,
  onStartAll,
  onStopAll,
}: ProjectSidebarProps) {
  const runningCount = projects.filter(
    (project) => snapshots[project.id]?.status === "running",
  ).length;

  return (
    <aside className="sidebar">
      <div className="brand">
        <div className="brand-mark"><span /></div>
        <div>
          <strong>Server Manager</strong>
          <small>{runningCount} de {projects.length} activos</small>
        </div>
      </div>

      <div className="global-actions">
        <button className="button button-primary button-small" onClick={onStartAll} disabled={projects.length === 0}>
          <PlayIcon /> Start All
        </button>
        <button className="button button-secondary button-small" onClick={onStopAll} disabled={runningCount === 0}>
          <StopIcon /> Stop All
        </button>
      </div>

      <div className="sidebar-heading">
        <span>PROYECTOS</span>
        <button className="icon-button" onClick={onCreate} aria-label="Nuevo proyecto" title="Nuevo proyecto">
          <PlusIcon />
        </button>
      </div>

      <nav className="project-list" aria-label="Proyectos">
        {projects.map((project) => {
          const status = snapshots[project.id]?.status ?? "stopped";
          return (
            <button
              className={`project-list-item${selectedId === project.id ? " selected" : ""}`}
              key={project.id}
              onClick={() => onSelect(project.id)}
            >
              <span className="project-icon">{project.icon || "▰"}</span>
              <span className="project-list-copy">
                <strong>{project.name}</strong>
                <small>{status === "running" ? `PID ${snapshots[project.id]?.pid}` : status}</small>
              </span>
              <StatusBadge status={status} compact />
            </button>
          );
        })}

        {projects.length === 0 && (
          <div className="sidebar-empty">
            <span>Ningún proyecto</span>
            <small>Crea el primero para comenzar.</small>
          </div>
        )}
      </nav>

      <button className="new-project-button" onClick={onCreate}>
        <PlusIcon /> Nuevo proyecto
      </button>
    </aside>
  );
}

