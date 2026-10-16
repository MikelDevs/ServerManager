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
      <p>Registra un servidor o aplicación local para controlar su proceso, ver sus logs y enviar comandos en tiempo real.</p>
      <button className="button button-primary" onClick={onCreate}><PlusIcon /> Crear primer proyecto</button>
    </main>
  );
}

