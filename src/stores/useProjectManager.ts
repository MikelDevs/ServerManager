import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { useCallback, useEffect, useMemo, useState } from "react";
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

  return first === 0 ? entries : entries.slice(first);
}

export function useProjectManager() {
  const [projects, setProjects] = useState<ProjectConfig[]>([]);
  const [snapshots, setSnapshots] = useState<Record<string, ProcessSnapshot>>({});
  const [logs, setLogs] = useState<ProjectLogs>({});
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [busyProjects, setBusyProjects] = useState<Set<string>>(new Set());
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const addSystemLog = useCallback((projectId: string, chunk: string) => {
    const event: ProcessOutputEvent = {
      projectId,
      stream: "system",
      chunk: chunk.endsWith("\n") ? chunk : `${chunk}\n`,
      timestamp: new Date().toISOString(),
    };
    setLogs((current) => ({
      ...current,
      [projectId]: trimLog([...(current[projectId] ?? []), event]),
    }));
  }, []);

  const refresh = useCallback(async () => {
    try {
      const [savedProjects, processStates] = await Promise.all([
        invoke<ProjectConfig[]>("list_projects"),
        invoke<ProcessSnapshot[]>("get_process_states"),
      ]);
      setProjects(savedProjects);
      setSnapshots(
        Object.fromEntries(processStates.map((snapshot) => [snapshot.projectId, snapshot])),
      );
      setSelectedId((current) =>
        current && savedProjects.some((project) => project.id === current)
          ? current
          : (savedProjects[0]?.id ?? null),
      );
      setError(null);
    } catch (caught) {
      setError(messageFromError(caught));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    let disposed = false;
    const unlisteners: UnlistenFn[] = [];

    void Promise.all([
      listen<ProcessOutputEvent>("process-output", ({ payload }) => {
        if (disposed) return;
        setLogs((current) => ({
          ...current,
          [payload.projectId]: trimLog([...(current[payload.projectId] ?? []), payload]),
        }));
      }),
      listen<ProcessStateEvent>("process-state", ({ payload }) => {
        if (disposed) return;
        setSnapshots((current) => ({
          ...current,
          [payload.projectId]: payload,
        }));
        if (payload.message) addSystemLog(payload.projectId, payload.message);
      }),
    ]).then((listeners) => {
      if (disposed) {
        listeners.forEach((unlisten) => unlisten());
      } else {
        unlisteners.push(...listeners);
      }
    });

    void refresh();

    return () => {
      disposed = true;
      unlisteners.forEach((unlisten) => unlisten());
    };
  }, [addSystemLog, refresh]);

  const runForProject = useCallback(
    async <T,>(projectId: string, command: string, args?: Record<string, unknown>) => {
      setBusyProjects((current) => new Set(current).add(projectId));
      setError(null);
      try {
        return await invoke<T>(command, { projectId, ...args });
      } catch (caught) {
        const message = messageFromError(caught);
        setError(message);
        addSystemLog(projectId, `Error: ${message}`);
        throw caught;
      } finally {
        setBusyProjects((current) => {
          const next = new Set(current);
          next.delete(projectId);
          return next;
        });
      }
    },
    [addSystemLog],
  );

  const saveProject = useCallback(
    async (input: ProjectInput) => {
      setError(null);
      const saved = await invoke<ProjectConfig>("save_project", { input });
      await refresh();
      setSelectedId(saved.id);
      return saved;
    },
    [refresh],
  );

  const deleteProject = useCallback(
    async (projectId: string) => {
      setError(null);
      try {
        await invoke("delete_project", { projectId });
        setLogs((current) => {
          const next = { ...current };
          delete next[projectId];
          return next;
        });
        await refresh();
      } catch (caught) {
        setError(messageFromError(caught));
        throw caught;
      }
    },
    [refresh],
  );

  const runAll = useCallback(async (command: "start_all" | "stop_all") => {
    setError(null);
    try {
      const results = await invoke<ActionResult[]>(command);
      const failures = results.filter((result) => !result.ok);
      if (failures.length > 0) {
        setError(failures.map((failure) => failure.message).join(" · "));
      }
      return results;
    } catch (caught) {
      setError(messageFromError(caught));
      throw caught;
    }
  }, []);

  const selectedProject = useMemo(
    () => projects.find((project) => project.id === selectedId) ?? null,
    [projects, selectedId],
  );

  return {
    projects,
    snapshots,
    logs,
    selectedId,
    selectedProject,
    busyProjects,
    loading,
    error,
    setSelectedId,
    clearError: () => setError(null),
    clearLogs: (projectId: string) =>
      setLogs((current) => ({ ...current, [projectId]: [] })),
    saveProject,
    deleteProject,
    startProject: (projectId: string) =>
      runForProject<ProcessSnapshot>(projectId, "start_project"),
    stopProject: (projectId: string) => runForProject<void>(projectId, "stop_project"),
    restartProject: (projectId: string) =>
      runForProject<ProcessSnapshot>(projectId, "restart_project"),
    sendInput: async (projectId: string, input: string) => {
      await runForProject<void>(projectId, "send_input", { input });
      addSystemLog(projectId, `› ${input}`);
    },
    openInCode: (projectId: string) =>
      runForProject<void>(projectId, "open_project_in_code"),
    openProjectUrl: (projectId: string) =>
      runForProject<void>(projectId, "open_project_url"),
    openProjectTerminal: async (projectId: string) => {
      try {
        await runForProject<void>(projectId, "open_ssh_terminal");
      } catch {
        // runForProject already displays the error and records it in the project log.
      }
    },
    startAll: () => runAll("start_all"),
    stopAll: () => runAll("stop_all"),
  };
}
