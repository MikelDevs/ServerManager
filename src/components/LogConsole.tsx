import { useEffect, useRef, useState, type FormEvent } from "react";
import type { ProcessOutputEvent, ProcessStatus } from "../types/project";
import { TerminalIcon, TrashIcon } from "./Icons";

interface LogConsoleProps {
  projectId: string;
  status: ProcessStatus;
  logs: ProcessOutputEvent[];
  onSendInput: (input: string) => Promise<unknown>;
  onClear: () => void;
}

export function LogConsole({ projectId, status, logs, onSendInput, onClear }: LogConsoleProps) {
  const [input, setInput] = useState("");
  const [sending, setSending] = useState(false);
  const outputRef = useRef<HTMLDivElement>(null);
  const shouldFollowRef = useRef(true);

  useEffect(() => {
    if (shouldFollowRef.current && outputRef.current) {
      outputRef.current.scrollTop = outputRef.current.scrollHeight;
    }
  }, [logs]);

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    const command = input.trim();
    if (!command || status !== "running") return;
    setSending(true);
    try {
      await onSendInput(command);
      setInput("");
    } finally {
      setSending(false);
    }
  };

  return (
    <section className="console-panel" aria-label={`Consola de ${projectId}`}>
      <header className="console-header">
        <div className="console-title">
          <TerminalIcon />
          <span>Consola</span>
          <small>{logs.length > 0 ? `${logs.length} bloques` : "Sin actividad"}</small>
        </div>
        <button className="button button-ghost button-small" onClick={onClear} disabled={logs.length === 0}>
          <TrashIcon /> Limpiar
        </button>
      </header>

      <div
        className="console-output"
        ref={outputRef}
        onScroll={(event) => {
          const element = event.currentTarget;
          shouldFollowRef.current = element.scrollHeight - element.scrollTop - element.clientHeight < 48;
        }}
      >
        {logs.length === 0 ? (
          <div className="console-placeholder">
            <TerminalIcon />
            <span>La salida del proceso aparecerá aquí.</span>
          </div>
        ) : (
          logs.map((entry, index) => (
            <span className={`log-chunk log-${entry.stream}`} key={`${entry.timestamp}-${index}`}>
              {entry.chunk}
            </span>
          ))
        )}
      </div>

      <form className="console-input" onSubmit={submit}>
        <span aria-hidden="true">›</span>
        <input
          value={input}
          onChange={(event) => setInput(event.target.value)}
          disabled={status !== "running" || sending}
          placeholder={status === "running" ? "Enviar comando a stdin…" : "Inicia el proyecto para enviar comandos"}
          aria-label="Comando para stdin"
          spellCheck={false}
        />
        <kbd>Enter</kbd>
      </form>
    </section>
  );
}

