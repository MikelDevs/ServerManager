import { useState } from "react";
import { CloseIcon } from "./components/Icons";
import { EmptyState } from "./components/EmptyState";
import { ProjectDetails } from "./components/ProjectDetails";
import { ProjectForm } from "./components/ProjectForm";
import { ProjectSidebar } from "./components/ProjectSidebar";
import { useProjectManager } from "./stores/useProjectManager";
import type { ProjectConfig } from "./types/project";

export default function App() {
  const manager = useProjectManager();
  const [formOpen, setFormOpen] = useState(false);
  const [editingProject, setEditingProject] = useState<ProjectConfig | null>(null);

  const createProject = () => {
    setEditingProject(null);
    setFormOpen(true);
  };

  const editProject = () => {
    setEditingProject(manager.selectedProject);
    setFormOpen(true);
  };

  const deleteSelectedProject = async () => {
    const project = manager.selectedProject;
    if (!project) return;
    if (window.confirm(`¿Eliminar “${project.name}”? Esta acción no elimina su carpeta.`)) {
      await manager.deleteProject(project.id);
    }
  };

  if (manager.loading) {
    return <div className="app-loading"><div className="loading-mark" /><span>Cargando Server Manager…</span></div>;
  }

  return (
    <div className="app-shell">
      <ProjectSidebar
        projects={manager.projects}
        snapshots={manager.snapshots}
        selectedId={manager.selectedId}
        onSelect={manager.setSelectedId}
        onCreate={createProject}
        busy={manager.busy}
        onStartAll={() => void manager.startAll().catch(() => {})}
        onStopAll={() => void manager.stopAll().catch(() => {})}
      />

      {manager.selectedProject ? (
        <ProjectDetails
          key={manager.selectedProject.id}
          project={manager.selectedProject}
          snapshots={manager.snapshots}
          logs={manager.logs}
          busy={manager.busyProjects.has(manager.selectedProject.id)}
          busyServices={manager.busyServices}
          onStart={() => manager.startProject(manager.selectedProject!.id)}
          onStop={() => manager.stopProject(manager.selectedProject!.id)}
          onRestart={() => manager.restartProject(manager.selectedProject!.id)}
          onStartService={manager.startService}
          onStopService={manager.stopService}
          onRestartService={manager.restartService}
          onSendInput={manager.sendInput}
          onClearLogs={manager.clearLogs}
          onEdit={editProject}
          onDelete={() => void deleteSelectedProject().catch(() => {})}
          onOpenInCode={() => manager.openInCode(manager.selectedProject!.id)}
          onOpenUrl={manager.openServiceUrl}
          onOpenTerminal={() => manager.openProjectTerminal(manager.selectedProject!.id)}
          onOpenServer={() => manager.openProjectServer(manager.selectedProject!.id)}
        />
      ) : (
        <EmptyState onCreate={createProject} />
      )}

      {manager.error && (
        <div className="error-toast" role="alert">
          <span>{manager.error}</span>
          <button onClick={manager.clearError} aria-label="Cerrar error"><CloseIcon /></button>
        </div>
      )}

      {formOpen && (
        <ProjectForm
          project={editingProject}
          onClose={() => setFormOpen(false)}
          onSave={manager.saveProject}
        />
      )}
    </div>
  );
}
