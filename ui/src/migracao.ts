import { invoke } from "@tauri-apps/api/core";

/**
 * Aviso da migração das configurações do CBA no Claude Code para as pastas dos
 * projetos (#305): espelho de `cba_session::migracao::AvisoMigracao`, campos em
 * camelCase.
 */
export interface AvisoMigracao {
  mensagem: string;
  /** Projetos preparados de novo, nas pastas. */
  projetos: string[];
  /** `.claude/settings.local.json` criados ou alterados. */
  arquivosLocais: string[];
  /** O `settings.json` do usuário. */
  settings: string;
  /** Eventos cujos hooks do app saíram do `settings.json`. */
  eventosRetirados: string[];
  /** Regras da proteção que saíram do `settings.json` (foram para as pastas). */
  regrasRetiradas: string[];
  /** Eventos em que ficou um hook do app que o analista mudou. */
  ficaram: string[];
  pendentes: { projeto: string; motivo: string }[];
}

export interface ApiMigracao {
  /** Migra, se ainda não migrou; devolve o aviso enquanto não foi visto. */
  migrar: () => Promise<AvisoMigracao | null>;
  marcarVisto: () => Promise<void>;
}

export const apiMigracaoTauri: ApiMigracao = {
  migrar: () => invoke("migrar_configuracoes_claude"),
  marcarVisto: () => invoke("marcar_aviso_migracao_visto"),
};
