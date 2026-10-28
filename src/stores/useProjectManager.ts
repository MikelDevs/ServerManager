import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type {
  ActionResult,
  ProcessOutputEvent,
  ProcessSnapshot,
  ProcessStateEvent,
  ProjectConfig,
  ProjectInput,
  ProjectLogs,
} from "../types/project";

const MAX_LOG_CHARACTERS = 500_000;

function messageFromError(error: unknown): string {
  if (typeof error === "string") return error;
  if (error instanceof Error) return error.message;
  return "Ocurrió un error inesperado";
}

function trimLog(entries: ProcessOutputEvent[]): ProcessOutputEvent[] {
  let characters = entries.reduce((total, entry) => total + entry.chunk.length, 0);
  let first = 0;
  while (characters > MAX_LOG_CHARACTERS && first < entries.length - 1) {
    characters -= entries[first].chunk.length;
    first += 1;
  }
  const trimmed = entries.slice(first);
  if (trimmed[0]?.chunk.length > MAX_LOG_CHARACTERS) {
    trimmed[0] = { ...trimmed[0], chunk: trimmed[0].chunk.slice(-MAX_LOG_CHARACTERS) };
  }
  return trimmed;
}

export function useProjectManager() {
  const [projects, setProjects] = useState<ProjectConfig[]>([]);
  const [snapshots, setSnapshots] = useState<Record<string, ProcessSnapshot>>({});
  const [logs, setLogs] = useState<ProjectLogs>({});
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [busyKeys, setBusyKeys] = useState<Set<string>>(new Set());
  const locks = useRef(new Set<string>());
  const stateVersions = useRef(new Map<string, number>());
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const addSystemLog = useCallback((serviceId: string, chunk: string) => {
    const event: ProcessOutputEvent = {
      serviceId,
      stream: "system",
      chunk: chunk.endsWith("\n") ? chunk : `${chunk}\n`,
      timestamp: new Date().toISOString(),
    };
    setLogs((current) => ({
      ...current,
      [serviceId]: trimLog([...(current[serviceId] ?? []), event]),
    }));
  }, []);

  const refresh = useCallback(async () => {
    const versionsAtStart = new Map(stateVersions.current);
    const [savedProjects, processStates] = await Promise.all([
      invoke<ProjectConfig[]>("list_projects"),
      invoke<ProcessSnapshot[]>("get_process_states"),
    ]);
    setProjects(savedProjects);
    setSnapshots((current) => Object.fromEntries(processStates.map((snapshot) => {
      const id = snapshot.serviceId;
      const receivedNewEvent = stateVersions.current.get(id) !== versionsAtStart.get(id);
      return [id, receivedNewEvent && current[id] ? current[id] : snapshot];
    })));
    setSelectedId((current) =>
      current && savedProjects.some((project) => project.id === current)
        ? current
        : (savedProjects[0]?.id ?? null),
    );
    const ids = new Set(savedProjects.flatMap((project) => project.services.map((service) => service.id)));
    setLogs((current) => Object.fromEntries(Object.entries(current).filter(([id]) => ids.has(id))));
  }, []);

  useEffect(() => {
    let disposed = false;
    const unlisteners: UnlistenFn[] = [];
    const initialize = async () => {
      try {
        // Register independently so a failed subscription cannot leak the other listener.
        for (const subscribe of [
          () => listen<ProcessOutputEvent>("process-output", ({ payload }) => {
            if (disposed) return;
            setLogs((current) => ({
              ...current,
              [payload.serviceId]: trimLog([...(current[payload.serviceId] ?? []), payload]),
            }));
          }),
          () => listen<ProcessStateEvent>("process-state", ({ payload }) => {
            if (disposed) return;
            stateVersions.current.set(payload.serviceId, (stateVersions.current.get(payload.serviceId) ?? 0) + 1);
            setSnapshots((current) => ({ ...current, [payload.serviceId]: payload }));
            if (payload.message) addSystemLog(payload.serviceId, payload.message);
          }),
        ]) {
          const unlisten = await subscribe();
          if (disposed) { unlisten(); return; }
          unlisteners.push(unlisten);
        }
        await refresh();
      } catch (caught) {
        if (!disposed) setError(messageFromError(caught));
      } finally {
        if (!disposed) setLoading(false);
      }
    };
    void initialize();
    return () => {
      disposed = true;
      unlisteners.forEach((unlisten) => unlisten());
    };
  }, [addSystemLog, refresh]);

  const run = useCallback(async <T,>(keys: string[], operation: () => Promise<T>, serviceId?: string) => {
    if (locks.current.has("all") || keys.some((key) => locks.current.has(key)) ||
        (keys.includes("all") && locks.current.size > 0)) {
      const message = "Espera a que termine la operación actual";
      setError(message);
      throw new Error(message);
    }
    keys.forEach((key) => locks.current.add(key));
    setBusyKeys(new Set(locks.current));
    setError(null);
    try {
      return await operation();
    } catch (caught) {
      const message = messageFromError(caught);
      setError(message);
      if (serviceId) addSystemLog(serviceId, `Error: ${message}`);
      throw caught;
    } finally {
      keys.forEach((key) => locks.current.delete(key));
      setBusyKeys(new Set(locks.current));
    }
  }, [addSystemLog]);

  const projectKeys = useCallback((projectId: string) => [
    `project:${projectId}`,
    ...(projects.find((project) => project.id === projectId)?.services.map((service) => `service:${service.id}`) ?? []),
  ], [projects]);

  const reportResults = useCallback((results: ActionResult[]) => {
    const failures = results.filter((result) => !result.ok);
    failures.forEach((failure) => addSystemLog(failure.serviceId, `Error: ${failure.message}`));
    if (failures.length) setError(failures.map((failure) => failure.message).join(" · "));
    return results;
  }, [addSystemLog]);

  const runGroup = (command: string, projectId?: string) => run(
    projectId ? projectKeys(projectId) : ["all"],
    async () => reportResults(await invoke<ActionResult[]>(command, projectId ? { projectId } : {})),
  );

  const runService = <T,>(serviceId: string, command: string, args?: Record<string, unknown>) =>
    run([`service:${serviceId}`], () => invoke<T>(command, { serviceId, ...args }), serviceId);

  const runProjectAction = (projectId: string, command: string) =>
    run(projectKeys(projectId), () => invoke<void>(command, { projectId }));

  const saveProject = (input: ProjectInput) => run(input.id ? projectKeys(input.id) : ["create"], async () => {
    const saved = await invoke<ProjectConfig>("save_project", { input });
    await refresh();
    setSelectedId(saved.id);
    return saved;
  });

  const deleteProject = (projectId: string) => run(projectKeys(projectId), async () => {
    await invoke("delete_project", { projectId });
    await refresh();
  });

  const selectedProject = useMemo(
    () => projects.find((project) => project.id === selectedId) ?? null,
    [projects, selectedId],
  );
  const busyServices = new Set(
    [...busyKeys].filter((key) => key.startsWith("service:")).map((key) => key.slice(8)),
  );
  const busyProjects = new Set(projects.filter((project) =>
    busyKeys.has("all") || busyKeys.has(`project:${project.id}`) ||
    project.services.some((service) => busyServices.has(service.id)),
  ).map((project) => project.id));

  return {
    projects, snapshots, logs, selectedId, selectedProject,
    busyProjects, busyServices, busy: busyKeys.size > 0, loading, error,
    setSelectedId,
    clearError: () => setError(null),
    clearLogs: (serviceId: string) => setLogs((current) => ({ ...current, [serviceId]: [] })),
    saveProject,
    deleteProject,
    startProject: (projectId: string) => runGroup("start_project", projectId),
    stopProject: (projectId: string) => runGroup("stop_project", projectId),
    restartProject: (projectId: string) => runGroup("restart_project", projectId),
    startService: (serviceId: string) => runService<ProcessSnapshot>(serviceId, "start_service"),
    stopService: (serviceId: string) => runService<void>(serviceId, "stop_service"),
    restartService: (serviceId: string) => runService<ProcessSnapshot>(serviceId, "restart_service"),
    sendInput: async (serviceId: string, input: string) => {
      await runService<void>(serviceId, "send_input", { input });
      addSystemLog(serviceId, `› ${input}`);
    },
    openInCode: (projectId: string) => runProjectAction(projectId, "open_project_in_code"),
    openProjectTerminal: (projectId: string) => runProjectAction(projectId, "open_project_terminal"),
    openProjectServer: (projectId: string) => runProjectAction(projectId, "open_ssh_terminal"),
    openServiceUrl: (serviceId: string) => runService<void>(serviceId, "open_service_url"),
    startAll: () => runGroup("start_all"),
    stopAll: () => runGroup("stop_all"),
  };
}
