import { open } from "@tauri-apps/plugin-dialog";
import { useEffect, useState, type FormEvent } from "react";
import type { ProjectConfig, ProjectInput } from "../types/project";
import { CloseIcon, FolderIcon, PlusIcon, TrashIcon } from "./Icons";

interface EnvironmentRow {
  id: string;
  key: string;
  value: string;
}

interface ProjectFormProps {
  project: ProjectConfig | null;
  onClose: () => void;
  onSave: (input: ProjectInput) => Promise<unknown>;
}

const newRow = (key = "", value = ""): EnvironmentRow => ({
  id: crypto.randomUUID(),
  key,
  value,
});

export function ProjectForm({ project, onClose, onSave }: ProjectFormProps) {
  const [name, setName] = useState("");
  const [icon, setIcon] = useState("🚀");
  const [workingDirectory, setWorkingDirectory] = useState("");
  const [startCommand, setStartCommand] = useState("");
  const [stopCommand, setStopCommand] = useState("");
  const [projectUrl, setProjectUrl] = useState("");
  const [sshIp, setSshIp] = useState("");
  const [environment, setEnvironment] = useState<EnvironmentRow[]>([]);
  const [saving, setSaving] = useState(false);
  const [browsing, setBrowsing] = useState(false);
  const [browseError, setBrowseError] = useState<string | null>(null);
  const [saveError, setSaveError] = useState<string | null>(null);

  useEffect(() => {
    setName(project?.name ?? "");
    setIcon(project?.icon ?? "🚀");
    setWorkingDirectory(project?.workingDirectory ?? "");
    setStartCommand(project?.startCommand ?? "");
    setStopCommand(project?.stopCommand ?? "");
    setProjectUrl(project?.projectUrl ?? "");
    setSshIp(project?.sshIp ?? "");
    setEnvironment(
      Object.entries(project?.environmentVariables ?? {}).map(([key, value]) => newRow(key, value)),
    );
    setBrowseError(null);
    setSaveError(null);
  }, [project]);

  const browseWorkingDirectory = async () => {
    setBrowsing(true);
    setBrowseError(null);
    try {
      const selected = await open({
        directory: true,
        multiple: false,
        title: "Seleccionar carpeta del proyecto",
        defaultPath: workingDirectory || undefined,
      });
      if (selected) {
        setWorkingDirectory(selected);
        if (!name.trim()) {
          const pathParts = selected.split(/[\\/]/).filter(Boolean);
          const folderName = pathParts[pathParts.length - 1];
          if (folderName) setName(folderName);
        }
      }
    } catch (error) {
      setBrowseError(error instanceof Error ? error.message : String(error));
    } finally {
      setBrowsing(false);
    }
  };

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    setSaving(true);
    setSaveError(null);
    try {
      await onSave({
        id: project?.id,
        name: name.trim(),
        icon: icon.trim() || null,
        workingDirectory: workingDirectory.trim(),
        startCommand: startCommand.trim(),
        stopCommand: stopCommand.trim() || null,
        sshIp: sshIp.trim() || null,
        projectUrl: projectUrl.trim()
          ? (/^[a-z][a-z\d+.-]*:\/\//i.test(projectUrl.trim())
              ? projectUrl.trim()
              : `http://${projectUrl.trim()}`)
          : null,
        environmentVariables: Object.fromEntries(
          environment
            .map((row) => [row.key.trim(), row.value] as const)
            .filter(([key]) => key.length > 0),
        ),
      });
      onClose();
    } catch (error) {
      setSaveError(typeof error === "string" ? error : error instanceof Error ? error.message : "No se pudo guardar el proyecto.");
    } finally {
      setSaving(false);
    }
  };

  const updateEnvironment = (id: string, field: "key" | "value", value: string) => {
    setEnvironment((current) =>
      current.map((row) => (row.id === id ? { ...row, [field]: value } : row)),
    );
  };

  return (
    <div className="modal-backdrop" role="presentation" onMouseDown={(event) => event.target === event.currentTarget && onClose()}>
      <section className="modal" role="dialog" aria-modal="true" aria-labelledby="project-form-title">
        <header className="modal-header">
          <div>
            <span className="eyebrow">CONFIGURACIÓN</span>
            <h2 id="project-form-title">{project ? "Editar proyecto" : "Nuevo proyecto"}</h2>
          </div>
          <button className="icon-button" onClick={onClose} aria-label="Cerrar"><CloseIcon /></button>
        </header>

        <form onSubmit={submit}>
          <div className="modal-body">
            <div className="form-grid name-grid">
              <label className="field icon-field">
                <span>Icono</span>
                <input value={icon} onChange={(event) => setIcon(event.target.value)} maxLength={8} placeholder="🚀" />
              </label>
              <label className="field">
                <span>Nombre</span>
                <input value={name} onChange={(event) => setName(event.target.value)} required autoFocus placeholder="API local" />
              </label>
            </div>

            <label className="field">
              <span>Carpeta de trabajo</span>
              <div className="path-picker">
                <input value={workingDirectory} onChange={(event) => setWorkingDirectory(event.target.value)} required placeholder="Selecciona la carpeta del proyecto" spellCheck={false} />
                <button type="button" className="button button-secondary browse-button" onClick={() => void browseWorkingDirectory()} disabled={browsing}>
                  <FolderIcon /> {browsing ? "Abriendo…" : "Examinar…"}
                </button>
              </div>
              {browseError ? <small className="field-error">No se pudo abrir el selector: {browseError}</small> : <small>Puedes examinar o pegar manualmente una ruta absoluta.</small>}
            </label>

            <label className="field">
              <span>Comando de inicio</span>
              <input className="code-input" value={startCommand} onChange={(event) => setStartCommand(event.target.value)} required placeholder="npm run dev" spellCheck={false} />
            </label>

            <label className="field">
              <span>Comando de parada por stdin <em>opcional</em></span>
              <input className="code-input" value={stopCommand} onChange={(event) => setStopCommand(event.target.value)} placeholder="stop" spellCheck={false} />
              <small>Se enviará como una línea al proceso antes de intentar terminarlo.</small>
            </label>

            <label className="field">
              <span>URL de la aplicación <em>opcional</em></span>
              <input className="code-input" type="text" inputMode="url" value={projectUrl} onChange={(event) => setProjectUrl(event.target.value)} placeholder="http://localhost:5173" spellCheck={false} />
              <small>Para proyectos React, Vite u otras aplicaciones web. Se abrirá en el navegador predeterminado.</small>
            </label>

            <label className="field">
              <span>IP para SSH <em>opcional</em></span>
              <input className="code-input" type="text" value={sshIp} onChange={(event) => setSshIp(event.target.value)} placeholder="203.0.113.10" autoComplete="off" autoCapitalize="none" spellCheck={false} />
              <small>“Abrir terminal” ejecutará ssh root@{sshIp.trim() || "<ip>"} con la IP guardada de este proyecto.</small>
            </label>

            <div className="environment-section">
              <div className="environment-heading">
                <div>
                  <span>Variables de entorno</span>
                  <small>Opcionales; se agregan al entorno heredado.</small>
                </div>
                <button type="button" className="button button-ghost button-small" onClick={() => setEnvironment((current) => [...current, newRow()])}>
                  <PlusIcon /> Agregar
                </button>
              </div>
              {environment.map((row) => (
                <div className="environment-row" key={row.id}>
                  <input className="code-input" value={row.key} onChange={(event) => updateEnvironment(row.id, "key", event.target.value)} placeholder="CLAVE" aria-label="Nombre de variable" spellCheck={false} />
                  <input className="code-input" value={row.value} onChange={(event) => updateEnvironment(row.id, "value", event.target.value)} placeholder="valor" aria-label="Valor de variable" spellCheck={false} />
                  <button type="button" className="icon-button danger" onClick={() => setEnvironment((current) => current.filter((item) => item.id !== row.id))} aria-label="Eliminar variable">
                    <TrashIcon />
                  </button>
                </div>
              ))}
              {environment.length === 0 && <div className="environment-empty">No hay variables configuradas.</div>}
            </div>
          </div>

          {saveError && <p className="form-save-error" role="alert">{saveError}</p>}
          <footer className="modal-footer">
            <button type="button" className="button button-secondary" onClick={onClose}>Cancelar</button>
            <button type="submit" className="button button-primary" disabled={saving}>{saving ? "Guardando…" : "Guardar proyecto"}</button>
          </footer>
        </form>
      </section>
    </div>
  );
}
