import { open } from "@tauri-apps/plugin-dialog";
import { useEffect, useRef, useState, type ChangeEvent, type FormEvent } from "react";
import type { ProjectConfig, ProjectInput } from "../types/project";
import { CloseIcon, FolderIcon, PlusIcon, TrashIcon } from "./Icons";
import { ProjectIcon } from "./ProjectIcon";
import { readProjectImage } from "../utils/projectImage";

interface EnvironmentRow { id: string; key: string; value: string; }
interface ServiceDraft {
  key: string;
  id?: string;
  name: string;
  kind: "api" | "app";
  workingDirectory: string;
  startCommand: string;
  stopCommand: string;
  projectUrl: string;
  environment: EnvironmentRow[];
}
interface ProjectFormProps {
  project: ProjectConfig | null;
  onClose: () => void;
  onSave: (input: ProjectInput) => Promise<unknown>;
}
const newRow = (key = "", value = ""): EnvironmentRow => ({ id: crypto.randomUUID(), key, value });
const newService = (kind: "api" | "app", number = 1): ServiceDraft => ({
  key: crypto.randomUUID(), name: `${kind === "api" ? "API" : "App"}${number > 1 ? ` ${number}` : ""}`,
  kind, workingDirectory: kind, startCommand: "npm run dev", stopCommand: "", projectUrl: "", environment: [],
});
function normalizeUrl(value: string): string | null {
  const trimmed = value.trim();
  if (!trimmed) return null;
  return /^[a-z][a-z\d+.-]*:\/\//i.test(trimmed) ? trimmed : `http://${trimmed}`;
}

export function ProjectForm({ project, onClose, onSave }: ProjectFormProps) {
  const [name, setName] = useState(project?.name ?? "");
  const [icon, setIcon] = useState(project?.icon ?? "");
  const [imageLoading, setImageLoading] = useState(false);
  const [imageError, setImageError] = useState<string | null>(null);
  const imageInput = useRef<HTMLInputElement>(null);
  const imageRequest = useRef(0);
  const [workingDirectory, setWorkingDirectory] = useState(project?.workingDirectory ?? "");
  const [sshIp, setSshIp] = useState(project?.sshIp ?? "");
  const [services, setServices] = useState<ServiceDraft[]>(() => project
    ? project.services.map((service) => ({ ...service, key: service.id, stopCommand: service.stopCommand ?? "", projectUrl: service.projectUrl ?? "", environment: Object.entries(service.environmentVariables).map(([key, value]) => newRow(key, value)) }))
    : [newService("api"), newService("app")]);
  const [saving, setSaving] = useState(false);
  const [browsing, setBrowsing] = useState(false);
  const [browseError, setBrowseError] = useState<string | null>(null);
  const [saveError, setSaveError] = useState<string | null>(null);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => { if (event.key === "Escape" && !saving && !browsing && !imageLoading) onClose(); };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [onClose, saving, browsing, imageLoading]);

  useEffect(() => () => { imageRequest.current += 1; }, []);

  const chooseImage = async (event: ChangeEvent<HTMLInputElement>) => {
    const file = event.currentTarget.files?.[0];
    event.currentTarget.value = "";
    if (!file) return;
    const request = ++imageRequest.current;
    setImageLoading(true);
    setImageError(null);
    try {
      const image = await readProjectImage(file);
      if (request === imageRequest.current) setIcon(image);
    } catch (error) {
      if (request === imageRequest.current) {
        setImageError(error instanceof Error ? error.message : "No se pudo cargar la imagen.");
      }
    } finally {
      if (request === imageRequest.current) setImageLoading(false);
    }
  };

  const updateService = (key: string, update: Partial<ServiceDraft>) => {
    setServices((current) => current.map((service) => service.key === key ? { ...service, ...update } : service));
  };
  const browseWorkingDirectory = async () => {
    setBrowsing(true); setBrowseError(null);
    try {
      const selected = await open({ directory: true, multiple: false, title: "Seleccionar carpeta raíz del proyecto", defaultPath: workingDirectory || undefined });
      if (selected) {
        setWorkingDirectory(selected);
        if (!name.trim()) {
          const pathParts = selected.split(/[\\/]/).filter(Boolean);
          const folderName = pathParts[pathParts.length - 1];
          if (folderName) setName(folderName);
        }
      }
    } catch (error) { setBrowseError(error instanceof Error ? error.message : String(error)); }
    finally { setBrowsing(false); }
  };
  const submit = async (event: FormEvent) => {
    event.preventDefault();
    if (saving || imageLoading || browsing) return;
    setSaving(true); setSaveError(null);
    try {
      if (services.length === 0) throw new Error("Agrega al menos una API o app al proyecto.");
      for (const service of services) {
        const keys = service.environment.map((row) => row.key.trim());
        if (keys.some((key) => !key)) throw new Error(`Completa el nombre de las variables de ${service.name}.`);
        if (new Set(keys).size !== keys.length) throw new Error(`Hay variables repetidas en ${service.name}.`);
      }
      await onSave({ id: project?.id, name: name.trim(), icon: icon.trim() || null, workingDirectory: workingDirectory.trim(), sshIp: sshIp.trim() || null,
        services: services.map((service) => ({ id: service.id, name: service.name.trim(), kind: service.kind, workingDirectory: service.workingDirectory.trim(), startCommand: service.startCommand.trim(), stopCommand: service.stopCommand.trim() || null, projectUrl: normalizeUrl(service.projectUrl), environmentVariables: Object.fromEntries(service.environment.map((row) => [row.key.trim(), row.value])) })),
      });
      onClose();
    } catch (error) { setSaveError(typeof error === "string" ? error : error instanceof Error ? error.message : "No se pudo guardar el proyecto."); }
    finally { setSaving(false); }
  };

  return (
    <div className="modal-backdrop" role="presentation" onMouseDown={(event) => event.target === event.currentTarget && !saving && !browsing && !imageLoading && onClose()}>
      <section className="modal project-modal" role="dialog" aria-modal="true" aria-labelledby="project-form-title">
        <header className="modal-header"><div><span className="eyebrow">PROYECTO Y SERVICIOS</span><h2 id="project-form-title">{project ? "Editar proyecto" : "Nuevo proyecto"}</h2></div><button className="icon-button" onClick={onClose} disabled={saving || browsing || imageLoading} aria-label="Cerrar"><CloseIcon /></button></header>
        <form onSubmit={submit}>
          <fieldset className="modal-body" disabled={saving}>
            <div className="project-image-picker" aria-busy={imageLoading}>
              <ProjectIcon icon={icon || null} name={name} className="project-image-preview" />
              <div className="project-image-options">
                <span>Imagen del proyecto</span>
                <div className="project-image-actions">
                  <button type="button" className="button button-secondary button-small" onClick={() => imageInput.current?.click()} disabled={imageLoading}>
                    {imageLoading ? "Cargando…" : icon ? "Cambiar imagen" : "Elegir imagen"}
                  </button>
                  {icon && <button type="button" className="button button-ghost button-small" onClick={() => { setIcon(""); setImageError(null); }} disabled={imageLoading}>Quitar</button>}
                </div>
                <small>PNG, JPG o WebP · máximo 5 MB</small>
                <input ref={imageInput} type="file" accept="image/png,image/jpeg,image/webp" onChange={(event) => void chooseImage(event)} disabled={imageLoading} aria-label="Seleccionar imagen del proyecto" hidden />
                {imageError && <small className="field-error" role="alert">{imageError}</small>}
              </div>
            </div>
            <label className="field"><span>Nombre del proyecto</span><input value={name} onChange={(event) => setName(event.target.value)} required autoFocus placeholder="RH" /></label>
            <label className="field"><span>Carpeta raíz</span><div className="path-picker"><input value={workingDirectory} onChange={(event) => setWorkingDirectory(event.target.value)} required placeholder="/ruta/a/rh" spellCheck={false} /><button type="button" className="button button-secondary browse-button" onClick={() => void browseWorkingDirectory()} disabled={browsing}><FolderIcon /> {browsing ? "Abriendo…" : "Examinar…"}</button></div>{browseError ? <small className="field-error">No se pudo abrir el selector: {browseError}</small> : <small>Ruta absoluta del proyecto. Abrir Code y Abrir terminal usarán esta carpeta.</small>}</label>
            <label className="field"><span>IP del servidor para SSH <em>opcional</em></span><input className="code-input" value={sshIp} onChange={(event) => setSshIp(event.target.value)} placeholder="203.0.113.10" autoComplete="off" autoCapitalize="none" spellCheck={false} /><small>“Abrir servidor” iniciará ssh root@{sshIp.trim() || "<ip>"} en una terminal.</small></label>
            <div className="services-form-heading"><div><h3>APIs y apps</h3><p>Cada servicio tiene su carpeta, comando y consola.</p></div><span className="service-count">{services.length} servicios</span></div>
            {services.map((service, index) => (
              <section className="service-editor" key={service.key} aria-label={`Configuración de ${service.name || `servicio ${index + 1}`}`}>
                <header className="service-editor-heading"><span className={`service-kind service-kind-${service.kind}`}>{service.kind === "api" ? "API" : "APP"}</span><strong>{service.name || `Servicio ${index + 1}`}</strong><button type="button" className="icon-button danger" onClick={() => setServices((current) => current.filter((item) => item.key !== service.key))} disabled={services.length === 1} title={services.length === 1 ? "El proyecto necesita al menos un servicio" : "Eliminar servicio"} aria-label={`Eliminar ${service.name}`}><TrashIcon /></button></header>
                <div className="service-editor-body">
                  <div className="form-grid service-name-grid"><label className="field"><span>Tipo</span><select value={service.kind} onChange={(event) => updateService(service.key, { kind: event.target.value as "api" | "app" })}><option value="api">API</option><option value="app">App</option></select></label><label className="field"><span>Nombre</span><input value={service.name} onChange={(event) => updateService(service.key, { name: event.target.value })} placeholder="API RH" required /></label></div>
                  <label className="field"><span>Carpeta del servicio</span><input className="code-input" value={service.workingDirectory} onChange={(event) => updateService(service.key, { workingDirectory: event.target.value })} placeholder={service.kind} spellCheck={false} required /><small>Relativa a la raíz, como {service.kind}, o una ruta absoluta. Usa . para ejecutar en la raíz.</small></label>
                  <label className="field"><span>Comando de inicio</span><input className="code-input" value={service.startCommand} onChange={(event) => updateService(service.key, { startCommand: event.target.value })} placeholder="npm run dev" spellCheck={false} required /></label>
                  <label className="field"><span>URL para abrir en el navegador <em>opcional</em></span><input className="code-input" inputMode="url" value={service.projectUrl} onChange={(event) => updateService(service.key, { projectUrl: event.target.value })} placeholder={service.kind === "api" ? "http://localhost:3000" : "http://localhost:5173"} spellCheck={false} /></label>
                  <details className="service-advanced"><summary>Parada y variables de entorno{service.environment.length > 0 ? ` (${service.environment.length})` : ""}</summary>
                    <label className="field"><span>Comando de parada por stdin <em>opcional</em></span><input className="code-input" value={service.stopCommand} onChange={(event) => updateService(service.key, { stopCommand: event.target.value })} placeholder="stop" spellCheck={false} /><small>Si está vacío, se solicitará la terminación del proceso.</small></label>
                    <div className="environment-section"><div className="environment-heading"><div><span>Variables de entorno</span><small>Se agregan al entorno de este servicio.</small></div><button type="button" className="button button-ghost button-small" onClick={() => updateService(service.key, { environment: [...service.environment, newRow()] })}><PlusIcon /> Agregar</button></div>
                      {service.environment.map((row) => (
                        <div className="environment-row" key={row.id}>
                          <input className="code-input" value={row.key} onChange={(event) => updateService(service.key, { environment: service.environment.map((item) => item.id === row.id ? { ...item, key: event.target.value } : item) })} placeholder="CLAVE" aria-label={`Nombre de variable de ${service.name}`} spellCheck={false} />
                          <input className="code-input" value={row.value} onChange={(event) => updateService(service.key, { environment: service.environment.map((item) => item.id === row.id ? { ...item, value: event.target.value } : item) })} placeholder="valor" aria-label={`Valor de variable de ${service.name}`} spellCheck={false} />
                          <button type="button" className="icon-button danger" onClick={() => updateService(service.key, { environment: service.environment.filter((item) => item.id !== row.id) })} aria-label={`Eliminar variable ${row.key}`}><TrashIcon /></button>
                        </div>
                      ))}
                      {service.environment.length === 0 && <div className="environment-empty">No hay variables configuradas.</div>}
                    </div>
                  </details>
                </div>
              </section>
            ))}
            <div className="add-services-actions"><button type="button" className="button button-secondary" onClick={() => setServices((current) => [...current, newService("api", current.filter((service) => service.kind === "api").length + 1)])}><PlusIcon /> Agregar API</button><button type="button" className="button button-secondary" onClick={() => setServices((current) => [...current, newService("app", current.filter((service) => service.kind === "app").length + 1)])}><PlusIcon /> Agregar app</button></div>
          </fieldset>
          {saveError && <p className="form-save-error" role="alert">{saveError}</p>}
          <footer className="modal-footer"><button type="button" className="button button-secondary" onClick={onClose} disabled={saving || browsing || imageLoading}>Cancelar</button><button type="submit" className="button button-primary" disabled={saving || browsing || imageLoading}>{saving ? "Guardando…" : "Guardar proyecto"}</button></footer>
        </form>
      </section>
    </div>
  );
}
