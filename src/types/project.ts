export type ProcessStatus = "running" | "stopping" | "stopped";
export type OutputStream = "stdout" | "stderr" | "system";
export type ServiceKind = "api" | "app";

export interface ServiceInput {
  id?: string;
  name: string;
  kind: ServiceKind;
  workingDirectory: string;
  startCommand: string;
  stopCommand: string | null;
  projectUrl: string | null;
  environmentVariables: Record<string, string>;
}

export interface ServiceConfig extends ServiceInput {
  id: string;
}

export interface ProjectInput {
  id?: string;
  name: string;
  workingDirectory: string;
  sshIp: string | null;
  icon: string | null;
  services: ServiceInput[];
}

export interface ProjectConfig extends ProjectInput {
  id: string;
  services: ServiceConfig[];
  createdAt: string;
  updatedAt: string;
}

export interface ProcessSnapshot {
  serviceId: string;
  status: ProcessStatus;
  pid: number | null;
  startedAt: string | null;
}

export interface ProcessStateEvent extends ProcessSnapshot {
  exitCode: number | null;
  message: string | null;
}

export interface ProcessOutputEvent {
  serviceId: string;
  stream: OutputStream;
  chunk: string;
  timestamp: string;
}

export interface ActionResult {
  serviceId: string;
  ok: boolean;
  message: string;
}

export type ProjectLogs = Record<string, ProcessOutputEvent[]>;
