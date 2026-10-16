export type ProcessStatus = "running" | "stopping" | "stopped";
export type OutputStream = "stdout" | "stderr" | "system";

export interface ProjectConfig {
  id: string;
  name: string;
  workingDirectory: string;
  startCommand: string;
  stopCommand: string | null;
  projectUrl: string | null;
  sshIp: string | null;
  environmentVariables: Record<string, string>;
  icon: string | null;
  createdAt: string;
  updatedAt: string;
}

export interface ProjectInput {
  id?: string;
  name: string;
  workingDirectory: string;
  startCommand: string;
  stopCommand: string | null;
  projectUrl: string | null;
  sshIp: string | null;
  environmentVariables: Record<string, string>;
  icon: string | null;
}

export interface ProcessSnapshot {
  projectId: string;
  status: ProcessStatus;
  pid: number | null;
  startedAt: string | null;
}

export interface ProcessStateEvent extends ProcessSnapshot {
  exitCode: number | null;
  message: string | null;
}

export interface ProcessOutputEvent {
  projectId: string;
  stream: OutputStream;
  chunk: string;
  timestamp: string;
}

export interface ActionResult {
  projectId: string;
  ok: boolean;
  message: string;
}

export type ProjectLogs = Record<string, ProcessOutputEvent[]>;
