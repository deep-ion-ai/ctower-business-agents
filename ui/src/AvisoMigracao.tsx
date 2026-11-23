import type { AvisoMigracao as Aviso } from "./migracao";

/**
 * Aviso único da migração das configurações do CBA no Claude Code para as pastas dos
 * projetos (#305), com a lista do que mudou e do que ficou como estava.
 */
export function AvisoMigracao({ aviso, aoFechar }: { aviso: Aviso; aoFechar: () => void }) {
  return (
    <div className="banner banner-info" role="status" aria-label="Configurações do Claude Code">
      <strong>{aviso.mensagem}.</strong>
      <ul>
        {aviso.projetos.length > 0 && (
          <li>Projetos com as configurações nas pastas: {aviso.projetos.join(", ")}.</li>
        )}
        {aviso.arquivosLocais.map((a) => (
          <li key={a}>
            Criado ou alterado: <code>{a}</code>
          </li>
        ))}
        {aviso.eventosRetirados.length > 0 && (
          <li>
            Saíram de <code>{aviso.settings}</code> os hooks do CBA em:{" "}
            {aviso.eventosRetirados.join(", ")}.
          </li>
        )}
        {aviso.regrasRetiradas.length > 0 && (
          <li>
            A proteção contra edição passou para as pastas; saíram de <code>{aviso.settings}</code>:{" "}
            {aviso.regrasRetiradas.map((r) => (
              <code key={r}>{r} </code>
            ))}
          </li>
        )}
        {aviso.ficaram.length > 0 && (
          <li>
            Ficaram em <code>{aviso.settings}</code>, porque foram mudados depois do CBA, os hooks
            de: {aviso.ficaram.join(", ")}. Confira e tire-os se não forem mais necessários.
          </li>
        )}
        {aviso.pendentes.map((p) => (
          <li key={p.projeto}>
            {p.projeto}: {p.motivo}; o ambiente é preparado ao abrir o terminal do projeto.
          </li>
        ))}
      </ul>
      <button type="button" onClick={aoFechar}>
        Entendi
      </button>
    </div>
  );
}
