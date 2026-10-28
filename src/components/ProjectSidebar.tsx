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
  busy?: boolean;
}

export function ProjectSidebar({
  projects,
  snapshots,
  selectedId,
  onSelect,
  onCreate,
  onStartAll,
  onStopAll,
  busy = false,
}: ProjectSidebarProps) {
  const services = projects.flatMap((project) => project.services);
  const runningCount = services.filter((service) => snapshots[service.id]?.status === "running").length;
  const activeCount = services.filter((service) => snapshots[service.id] && snapshots[service.id].status !== "stopped").length;

  return (
    <aside className="sidebar">
      <div className="brand">
        <div className="brand-mark"><span /></div>
        <div>
          <strong>Server Manager</strong>
          <small>{runningCount} de {services.length} servicios activos</small>
        </div>
      </div>

      <div className="global-actions">
        <button className="button button-primary button-small" onClick={onStartAll} disabled={services.length === 0 || runningCount === services.length || busy}>
          <PlayIcon /> Iniciar proyectos
        </button>
        <button className="button button-secondary button-small" onClick={onStopAll} disabled={activeCount === 0 || busy}>
          <StopIcon /> Detener proyectos
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
          const running = project.services.filter((service) => snapshots[service.id]?.status === "running").length;
          const stopping = project.services.some((service) => snapshots[service.id]?.status === "stopping");
          const status = stopping ? "stopping" : running > 0 ? "running" : "stopped";
          return (
            <button
              className={`project-list-item${selectedId === project.id ? " selected" : ""}`}
              key={project.id}
              onClick={() => onSelect(project.id)}
            >
              <span className="project-icon">{project.icon || "▰"}</span>
              <span className="project-list-copy">
                <strong>{project.name}</strong>
                <small>{stopping ? "Deteniendo…" : `${running}/${project.services.length} servicios activos`}</small>
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
      <div className="sidebar-version">Server Manager · 0.4.0</div>
    </aside>
  );
}

