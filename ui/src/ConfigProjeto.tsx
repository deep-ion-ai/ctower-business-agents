import { useEffect, useId, useRef, useState, type KeyboardEvent } from "react";
import { AbaAgente } from "./AbaAgente";
import { AbaAvancado } from "./AbaAvancado";
import { AbaCodigoDemandas } from "./AbaCodigoDemandas";
import { AbaLeitura } from "./AbaLeitura";
import { AbaVisaoGeral } from "./AbaVisaoGeral";
import type {
  AmbienteDetalhado,
  ApiConfiguracoes,
  Configuracoes,
  ProjetoConfig,
  TextoEditavel,
} from "./configuracoes";
import { DialogoConfirmacao } from "./DialogoConfirmacao";
import {
  ABAS_DO_PROJETO,
  itensDeSaude,
  marcasDasAbas,
  resumoDoProjeto,
  type AbaProjeto,
  type EstadoDoProjeto,
} from "./saudeProjeto";

export interface PropsConfigProjeto {
  api: ApiConfiguracoes;
  cfg: Pick<Configuracoes, "configClaude" | "base">;
  projeto: ProjetoConfig;
  textos: TextoEditavel[];
  /** Ambiente e prontidão já conhecidos (verificados uma vez por abertura da tela). */
  estado: EstadoDoProjeto;
  /** O cadastro mudou (repositórios, pastas, descadastro): a tela relê as configurações. */
  aoMudar: () => void;
  aoAbrirTexto: () => void;
  aoEditar: (arquivo: string) => void;
  /** Verifica o ambiente de novo, depois de uma ação que o muda. */
  aoReverificarAmbiente: (projeto: string) => Promise<AmbienteDetalhado | null>;
  /** Confere a prontidão dos repositórios (de novo, se `forcar`). */
  aoConferirProntidao: (projeto: string, forcar?: boolean) => void;
  /** Leva para a seção Acessos (tokens globais dos hosts). */
  aoIrParaAcessos: () => void;
  /** Há (ou deixou de haver) alterações não salvas em “O que é analisado”. */
  aoPendencia?: (pendente: boolean) => void;
}

const SELO = {
  ok: "✓ Tudo pronto",
} as const;

/**
 * Página do projeto nas configurações (#192): cabeçalho com nome, selo de saúde e
 * resumo, e as abas Visão geral, Código e demandas, O que é analisado, Agente e
 * Avançado. As abas seguem o padrão WAI-ARIA (setas, Home e End; uma só aba no Tab).
 * Sair de “O que é analisado” com alterações não salvas pede confirmação.
 */
export function ConfigProjeto(props: PropsConfigProjeto) {
  const { projeto, estado, aoConferirProntidao, aoPendencia } = props;
  const [aba, setAba] = useState<AbaProjeto>("geral");
  const [pendente, setPendente] = useState(false);
  const [saida, setSaida] = useState<AbaProjeto | null>(null);
  // O `:` do useId atrapalha seletores de id.
  const base = useId().replace(/:/g, "");
  const botoes = useRef<Partial<Record<AbaProjeto, HTMLButtonElement | null>>>({});

  // A prontidão dos repositórios é conferida sozinha ao abrir a página.
  useEffect(() => {
    aoConferirProntidao(projeto.id);
  }, [aoConferirProntidao, projeto.id]);

  const pendencia = useRef(aoPendencia);
  pendencia.current = aoPendencia;
  useEffect(() => {
    pendencia.current?.(pendente);
  }, [pendente]);
  useEffect(() => () => pendencia.current?.(false), []);

  const itens = itensDeSaude(projeto, estado);
  const marcas = marcasDasAbas(itens);
  const problemas = itens.filter((i) => i.estado === "problema").length;
  const verificando = estado.ambiente === undefined;

  const irPara = (nova: AbaProjeto) => {
    if (nova === aba) return;
    if (aba === "leitura" && pendente) {
      setSaida(nova);
      return;
    }
    setAba(nova);
  };

  const teclado = (e: KeyboardEvent<HTMLButtonElement>) => {
    const atual = ABAS_DO_PROJETO.findIndex((a) => a.id === aba);
    const n = ABAS_DO_PROJETO.length;
    const destino =
      e.key === "ArrowRight"
        ? (atual + 1) % n
        : e.key === "ArrowLeft"
          ? (atual - 1 + n) % n
          : e.key === "Home"
            ? 0
            : e.key === "End"
              ? n - 1
              : -1;
    if (destino < 0) return;
    e.preventDefault();
    const nova = ABAS_DO_PROJETO[destino].id;
    irPara(nova);
    botoes.current[nova]?.focus();
  };

  const idAba = (a: AbaProjeto) => `${base}-aba-${a}`;
  const idPainel = (a: AbaProjeto) => `${base}-painel-${a}`;

  return (
    <section className="config-secao pagina-projeto" aria-label={`Projeto ${projeto.nome}`}>
      <header className="pagina-projeto-topo">
        <div className="pagina-projeto-titulo">
          <h3>{projeto.nome}</h3>
          <span
            className="badge"
            data-tone={verificando ? undefined : problemas > 0 ? "attention" : "done"}
          >
            {verificando
              ? "Verificando…"
              : problemas > 0
                ? `${problemas} ${problemas === 1 ? "pendência" : "pendências"}`
                : SELO.ok}
          </span>
        </div>
        <p className="muted">{resumoDoProjeto(projeto)}</p>
        <div className="abas-projeto" role="tablist" aria-label="Partes do projeto">
          {ABAS_DO_PROJETO.map((a) => {
            const marca = marcas[a.id];
            return (
              <button
                key={a.id}
                ref={(el) => {
                  botoes.current[a.id] = el;
                }}
                id={idAba(a.id)}
                type="button"
                role="tab"
                className="aba-projeto"
                aria-selected={aba === a.id}
                aria-controls={idPainel(a.id)}
                tabIndex={aba === a.id ? 0 : -1}
                onClick={() => irPara(a.id)}
                onKeyDown={teclado}
              >
                {a.rotulo}
                {marca && (
                  <span className="marca-aba">
                    {marca}
                    <span className="sr-only">
                      {marca === "!"
                        ? " (com pendência)"
                        : ` ${marca === "1" ? "pendência" : "pendências"}`}
                    </span>
                  </span>
                )}
              </button>
            );
          })}
        </div>
      </header>
      <div
        id={idPainel(aba)}
        role="tabpanel"
        aria-labelledby={idAba(aba)}
        className="painel-projeto"
        tabIndex={0}
      >
        {aba === "geral" && <AbaVisaoGeral itens={itens} aoIr={irPara} />}
        {aba === "codigo" && <AbaCodigoDemandas {...props} />}
        {aba === "leitura" && <AbaLeitura {...props} aoPendencia={setPendente} />}
        {aba === "agente" && <AbaAgente {...props} />}
        {aba === "avancado" && <AbaAvancado {...props} />}
      </div>
      {saida && (
        <DialogoConfirmacao
          titulo="Sair sem salvar?"
          aoCancelar={() => setSaida(null)}
          acoes={
            <>
              <button className="btn" type="button" autoFocus onClick={() => setSaida(null)}>
                Cancelar
              </button>
              <button
                className="btn btn-danger"
                type="button"
                onClick={() => {
                  setPendente(false);
                  setAba(saida);
                  setSaida(null);
                }}
              >
                Descartar as alterações
              </button>
            </>
          }
        >
          <p>As alterações em “O que é analisado” ainda não foram salvas e serão perdidas.</p>
        </DialogoConfirmacao>
      )}
    </section>
  );
}
