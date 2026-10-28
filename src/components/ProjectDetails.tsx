import { useEffect, useState, type KeyboardEvent } from "react";
import type { ProcessSnapshot, ProjectConfig, ProjectLogs } from "../types/project";
import { BrowserIcon, ClockIcon, CodeIcon, EditIcon, FolderIcon, PlayIcon, RestartIcon, ServerIcon, StopIcon, TerminalIcon, TrashIcon } from "./Icons";
import { LogConsole } from "./LogConsole";
import { StatusBadge } from "./StatusBadge";

interface ProjectDetailsProps {
  project: ProjectConfig;
  snapshots: Record<string, ProcessSnapshot>;
  logs: ProjectLogs;
  busy: boolean;
  busyServices: Set<string>;
  onStart: () => Promise<unknown>;
  onStop: () => Promise<unknown>;
  onRestart: () => Promise<unknown>;
  onStartService: (serviceId: string) => Promise<unknown>;
  onStopService: (serviceId: string) => Promise<unknown>;
  onRestartService: (serviceId: string) => Promise<unknown>;
  onSendInput: (serviceId: string, input: string) => Promise<void>;
  onClearLogs: (serviceId: string) => void;
  onEdit: () => void;
  onDelete: () => void;
  onOpenInCode: () => Promise<unknown>;
  onOpenUrl: (serviceId: string) => Promise<unknown>;
  onOpenTerminal: () => Promise<unknown>;
  onOpenServer: () => Promise<unknown>;
}
function formatUptime(startedAt: string | null | undefined, now: number): string {
  if (!startedAt) return "—";
  const seconds = Math.max(0, Math.floor((now - new Date(startedAt).getTime()) / 1000));
  const hours = Math.floor(seconds / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  const remainder = seconds % 60;
  return hours > 0 ? `${hours}h ${minutes.toString().padStart(2, "0")}m ${remainder.toString().padStart(2, "0")}s` : `${minutes}m ${remainder.toString().padStart(2, "0")}s`;
}
// Errors are displayed by the project manager; event handlers should not leak rejections.
const run = (action: () => Promise<unknown>) => { void action().catch(() => {}); };

export function ProjectDetails({ project, snapshots, logs, busy, busyServices, onStart, onStop, onRestart, onStartService, onStopService, onRestartService, onSendInput, onClearLogs, onEdit, onDelete, onOpenInCode, onOpenUrl, onOpenTerminal, onOpenServer }: ProjectDetailsProps) {
  const [selectedServiceId, setSelectedServiceId] = useState<string | null>(null);
  const [now, setNow] = useState(Date.now());
  const selected = project.services.find((service) => service.id === selectedServiceId) ?? project.services[0];
  const running = project.services.filter((service) => snapshots[service.id]?.status === "running").length;
  const active = project.services.filter((service) => snapshots[service.id] && snapshots[service.id].status !== "stopped").length;
  const anyBusy = busy || project.services.some((service) => busyServices.has(service.id));
  const stopping = project.services.some((service) => snapshots[service.id]?.status === "stopping");
  const status = selected ? snapshots[selected.id]?.status ?? "stopped" : "stopped";
  const snapshot = selected ? snapshots[selected.id] : undefined;
  const selectedBusy = busy || (selected ? busyServices.has(selected.id) : false);
  const selectWithKeyboard = (event: KeyboardEvent<HTMLDivElement>) => {
    const keys = ["ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown", "Home", "End"];
    if (!keys.includes(event.key) || !project.services.length) return;
    event.preventDefault();
    const current = project.services.findIndex((service) => service.id === selected?.id);
    const last = project.services.length - 1;
    const next = event.key === "Home" ? 0 : event.key === "End" ? last
      : (current + (["ArrowLeft", "ArrowUp"].includes(event.key) ? -1 : 1) + project.services.length) % project.services.length;
    setSelectedServiceId(project.services[next].id);
    event.currentTarget.querySelectorAll<HTMLButtonElement>('[role="tab"]')[next]?.focus();
  };
  useEffect(() => { setSelectedServiceId(null); }, [project.id]);
  useEffect(() => {
    if (!running) return;
    setNow(Date.now());
    const interval = window.setInterval(() => setNow(Date.now()), 1_000);
    return () => window.clearInterval(interval);
  }, [running]);

  return (
    <main className="main-panel">
      <header className="project-header">
        <div className="project-identity"><span className="project-hero-icon">{project.icon || "▰"}</span><div className="project-identity-copy"><div className="project-title-row"><h1>{project.name}</h1><span className="project-service-count">{running}/{project.services.length} activos</span></div><div className="project-path" title={project.workingDirectory}><FolderIcon /> {project.workingDirectory}</div></div></div>
        <div className="header-actions">
          <button className="button button-secondary button-small" onClick={() => run(onOpenInCode)} title="Abrir la raíz del proyecto en Visual Studio Code" disabled={busy}><CodeIcon /> Abrir Code</button>
          <button className="button button-secondary button-small" onClick={() => run(onOpenTerminal)} title="Abrir una terminal en la raíz del proyecto" disabled={busy}><TerminalIcon /> Abrir terminal</button>
          <button className="button button-secondary button-small" onClick={() => run(onOpenServer)} title={project.sshIp ? `ssh root@${project.sshIp}` : "Configura la IP del servidor al editar este proyecto"} disabled={!project.sshIp || busy}><ServerIcon /> Abrir servidor</button>
          <button className="icon-button" onClick={onEdit} title={active > 0 ? "Detén los servicios antes de editar" : "Editar proyecto"} aria-label="Editar proyecto" disabled={active > 0 || anyBusy}><EditIcon /></button>
          <button className="icon-button danger" onClick={onDelete} title={active > 0 ? "Detén los servicios antes de eliminar" : "Eliminar proyecto"} aria-label="Eliminar proyecto" disabled={active > 0 || anyBusy}><TrashIcon /></button>
        </div>
      </header>
      <div className="control-strip project-controls"><div className="process-actions">
        <button className="button button-success" onClick={() => run(onStart)} disabled={running === project.services.length || stopping || anyBusy}><PlayIcon /> Iniciar todo</button>
        <button className="button button-secondary" onClick={() => run(onStop)} disabled={active === 0 || anyBusy}><StopIcon /> Detener todo</button>
        <button className="button button-secondary" onClick={() => run(onRestart)} disabled={active === 0 || anyBusy}><RestartIcon /> Reiniciar todo</button>
      </div><span className="group-action-hint">Servicios de {project.name}</span></div>
      <section className="services-section" aria-label="Servicios del proyecto">
        <div className="services-heading"><span>APIs Y APPS</span><small>Selecciona un servicio para ver su consola</small></div>
        <div className="service-list" role="tablist" aria-label="Consolas por servicio" onKeyDown={selectWithKeyboard}>
          {project.services.map((service) => <button key={service.id} id={`tab-${service.id}`} role="tab" tabIndex={selected?.id === service.id ? 0 : -1} aria-selected={selected?.id === service.id} aria-controls="service-detail" className={`service-card${selected?.id === service.id ? " selected" : ""}`} onClick={() => setSelectedServiceId(service.id)}><span className={`service-kind service-kind-${service.kind}`}>{service.kind === "api" ? "API" : "APP"}</span><span className="service-card-copy"><strong>{service.name}</strong><small title={service.workingDirectory}>{service.workingDirectory}</small></span><StatusBadge status={snapshots[service.id]?.status ?? "stopped"} /></button>)}
        </div>
      </section>
      {selected && <section className="service-detail" id="service-detail" role="tabpanel" aria-labelledby={`tab-${selected.id}`}>
        <div className="service-detail-header"><div><h2>{selected.name}</h2><span className="service-runtime">PID {snapshot?.pid ?? "—"}<span>·</span><ClockIcon /> {status === "running" ? formatUptime(snapshot?.startedAt, now) : "Sin actividad"}</span></div><div className="process-actions service-actions">
          <button className="button button-success button-small" onClick={() => run(() => onStartService(selected.id))} disabled={status !== "stopped" || selectedBusy}><PlayIcon /> Iniciar</button>
          <button className="button button-secondary button-small" onClick={() => run(() => onStopService(selected.id))} disabled={status === "stopped" || selectedBusy}><StopIcon /> Detener</button>
          <button className="button button-secondary button-small" onClick={() => run(() => onRestartService(selected.id))} disabled={status === "stopped" || selectedBusy} aria-label={`Reiniciar ${selected.name}`} title="Reiniciar servicio"><RestartIcon /></button>
          <button className="button button-secondary button-small" onClick={() => run(() => onOpenUrl(selected.id))} disabled={!selected.projectUrl || selectedBusy} title={selected.projectUrl ?? "Configura una URL para este servicio"}><BrowserIcon /> Abrir {selected.kind === "api" ? "API" : "app"}</button>
        </div></div>
        <section className="config-summary" aria-label="Configuración del servicio"><div><span>COMANDO DE INICIO</span><code title={selected.startCommand}>{selected.startCommand}</code></div><div><span>PARADA POR STDIN</span><code title={selected.stopCommand ?? undefined}>{selected.stopCommand || "Terminación del sistema"}</code></div><div><span>ENTORNO</span><strong>{Object.keys(selected.environmentVariables).length} variables</strong></div></section>
        <LogConsole key={selected.id} serviceId={selected.id} serviceName={selected.name} status={status} logs={logs[selected.id] ?? []} onSendInput={(input) => onSendInput(selected.id, input)} onClear={() => onClearLogs(selected.id)} />
      </section>}
    </main>
  );
}
