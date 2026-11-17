import type { KeyboardEvent } from "react";
import { GRUPOS_DO_MENU, SECOES_OCULTAS, type Escolha, type SecaoConfig } from "./configSecoes";

/** Projeto no menu: `pendencias` é `null` enquanto a verificação não terminou. */
export interface ProjetoNoMenu {
  id: string;
  nome: string;
  pendencias: number | null;
}

/**
 * Menu lateral das configurações (#191): App, Base de conhecimento e Projetos (um item
 * por projeto, com ponto de status e nº de pendências, mais “+ Novo projeto”). O item
 * aberto leva `aria-current="page"`; as setas para cima e para baixo andam pelos itens.
 */
export function MenuConfiguracoes({
  escolha,
  projetos,
  podeCadastrar,
  aoEscolher,
  aoNovoProjeto,
}: {
  escolha: Escolha;
  projetos: ProjetoNoMenu[];
  /** Há base escolhida: sem ela não dá para cadastrar projeto. */
  podeCadastrar: boolean;
  aoEscolher: (e: Escolha) => void;
  aoNovoProjeto: () => void;
}) {
  const atual = (s: SecaoConfig) => (escolha.secao === s ? ("page" as const) : undefined);

  const teclas = (e: KeyboardEvent<HTMLElement>) => {
    if (e.key !== "ArrowDown" && e.key !== "ArrowUp") return;
    const itens = Array.from(
      e.currentTarget.querySelectorAll<HTMLButtonElement>("button[data-item-menu]:not(:disabled)"),
    );
    const i = itens.findIndex((b) => b === document.activeElement);
    if (i < 0) return;
    e.preventDefault();
    const proximo = (i + (e.key === "ArrowDown" ? 1 : -1) + itens.length) % itens.length;
    itens[proximo].focus();
  };

  return (
    <nav className="menu-config" aria-label="Seções das configurações" onKeyDown={teclas}>
      {GRUPOS_DO_MENU.map((g) => (
        <div key={g.titulo} className="menu-config-grupo" role="group" aria-label={g.titulo}>
          <span className="menu-config-titulo" aria-hidden="true">
            {g.titulo}
          </span>
          {g.secoes
            .filter((s) => !SECOES_OCULTAS.includes(s.id))
            .map((s) => (
              <button
                key={s.id}
                type="button"
                className="menu-config-item"
                data-item-menu=""
                aria-current={atual(s.id)}
                onClick={() => aoEscolher({ secao: s.id, projeto: null })}
              >
                {s.rotulo}
              </button>
            ))}
        </div>
      ))}
      <div className="menu-config-grupo" role="group" aria-label="Projetos">
        <span className="menu-config-titulo" aria-hidden="true">
          Projetos
        </span>
        {projetos.length === 0 && <p className="muted menu-config-vazio">Nenhum projeto cadastrado.</p>}
        {projetos.map((p) => {
          const aberto = escolha.secao === "projeto" && escolha.projeto === p.id;
          const pendente = (p.pendencias ?? 0) > 0;
          return (
            <button
              key={p.id}
              type="button"
              className="menu-config-item"
              data-item-menu=""
              aria-current={aberto ? "page" : undefined}
              onClick={() => aoEscolher({ secao: "projeto", projeto: p.id })}
            >
              <span
                className="ponto-status"
                data-estado={p.pendencias === null ? "verificando" : pendente ? "pendente" : "ok"}
                aria-hidden="true"
              />
              <span className="menu-config-nome">{p.nome}</span>
              {pendente && (
                <span className="menu-config-contagem">
                  {p.pendencias}
                  <span className="sr-only"> {p.pendencias === 1 ? "pendência" : "pendências"}</span>
                </span>
              )}
            </button>
          );
        })}
        <button
          type="button"
          className="btn btn-sm menu-config-novo"
          data-item-menu=""
          disabled={!podeCadastrar}
          onClick={aoNovoProjeto}
        >
          + Novo projeto
        </button>
        {!podeCadastrar && (
          <p className="muted menu-config-vazio">Escolha a pasta da base para cadastrar projetos.</p>
        )}
      </div>
    </nav>
  );
}
