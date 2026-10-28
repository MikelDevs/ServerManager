import { PlusIcon, TerminalIcon } from "./Icons";

export function EmptyState({ onCreate }: { onCreate: () => void }) {
  return (
    <main className="main-panel empty-state">
      <div className="empty-illustration">
        <TerminalIcon />
        <span className="pulse-dot" />
      </div>
      <span className="eyebrow">LISTO PARA COMENZAR</span>
      <h1>Administra tus proyectos<br />desde un solo lugar.</h1>
      <p>Agrupa las APIs y apps de cada proyecto. Inícialas juntas, consulta sus consolas y abre tu carpeta de trabajo o servidor.</p>
      <button className="button button-primary" onClick={onCreate}><PlusIcon /> Crear primer proyecto</button>
    </main>
  );
}

