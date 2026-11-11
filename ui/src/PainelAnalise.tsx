import { useCallback, useRef, useState } from "react";
import { intervaloDe, mensagemDe, type ApiAnalise, type Evidencia } from "./analise";
import { chaveEvidencia, DocumentoAoVivo } from "./DocumentoAoVivo";
import { PromptsProntos } from "./PromptsProntos";
import { TerminalClaude, type ControleTerminal, type EstadoTerminal } from "./TerminalClaude";
import { TrechoEvidencia, type CargaTrecho } from "./TrechoEvidencia";
import type { ApiTerminal, PedidoClaude } from "./terminal";
import type { TelaTerminal } from "./telaXterm";
import { useAnaliseAoVivo } from "./useAnaliseAoVivo";

/** Como a tela chegou à análise. */
export type Abertura =
  /** Análise recém-criada: a conversa começa com o pedido de `iniciar_analise`. */
  | { tipo: "nova"; promptInicial: string }
  /** Análise existente: o analista escolhe retomar a conversa ou começar outra. */
  | { tipo: "reabrir" };

interface Props {
  api: ApiAnalise;
  sistema: string;
  analise: string;
  abertura: Abertura;
  aoVoltar: () => void;
  /** Para testes. */
  apiTerminal?: ApiTerminal;
  criarTela?: () => TelaTerminal;
}

/**
 * Tela da análise: o terminal do Claude Code de um lado e, do outro, o documento
 * montado ao vivo, com o trecho da evidência escolhida (spec, seções 6.4 e 10).
 * Depois de "Continuar no Claude Desktop", o terminal some e fica o documento, com o
 * botão para voltar à conversa aqui.
 */
export function PainelAnalise({
  api,
  sistema,
  analise,
  abertura,
  aoVoltar,
  apiTerminal,
  criarTela,
}: Props) {
  const { vista, erro } = useAnaliseAoVivo(api, sistema, analise);
  const [conversa, setConversa] = useState<{ chave: number; pedido: PedidoClaude } | null>(
    abertura.tipo === "nova"
      ? { chave: 1, pedido: { promptInicial: abertura.promptInicial } }
      : null,
  );
  const [noDesktop, setNoDesktop] = useState(false);
  const [estadoTerminal, setEstadoTerminal] = useState<EstadoTerminal | null>(null);
  const [trecho, setTrecho] = useState<CargaTrecho | null>(null);
  const [falhaEnvio, setFalhaEnvio] = useState<string | null>(null);
  const controle = useRef<ControleTerminal>(null);
  const vivo = estadoTerminal?.fase === "aberto";

  const conversar = useCallback((pedido: PedidoClaude) => {
    setNoDesktop(false);
    setConversa((c) => ({ chave: (c?.chave ?? 0) + 1, pedido }));
  }, []);

  const retomar = useCallback(() => {
    const id = vista?.sessao?.id;
    conversar({ retomada: id ? { modo: "sessao", id } : { modo: "continuar" } });
  }, [conversar, vista]);

  const enviar = (texto: string) => {
    setFalhaEnvio(null);
    controle.current?.enviarPrompt(texto).catch((e: unknown) => setFalhaEnvio(mensagemDe(e)));
  };

  const continuarNoDesktop = () => {
    setFalhaEnvio(null);
    controle.current
      ?.continuarNoDesktop()
      .then(() => setNoDesktop(true))
      .catch((e: unknown) => setFalhaEnvio(mensagemDe(e)));
  };

  const voltarAConversa = () => {
    if (vivo) setNoDesktop(false);
    else retomar();
  };

  const abrirEvidencia = (e: Evidencia) => {
    const chave = chaveEvidencia(e);
    const { inicio, fim } = intervaloDe(e.linhas);
    setTrecho({ tipo: "carregando", chave });
    api
      .trechoEvidencia(sistema, e.arquivo, inicio, fim)
      .then((t) =>
        setTrecho((atual) =>
          atual?.chave === chave ? { tipo: "pronto", chave, trecho: t } : atual,
        ),
      )
      .catch((err: unknown) =>
        setTrecho((atual) =>
          atual?.chave === chave ? { tipo: "erro", chave, mensagem: mensagemDe(err) } : atual,
        ),
      );
  };

  return (
    <div className="painel-analise">
      <div className="painel-barra">
        <button type="button" onClick={aoVoltar}>
          ← Análises
        </button>
        <strong>{analise}</strong>
      </div>
      <div className="painel-colunas">
        <section className="painel-conversa" aria-label="Conversa">
          {noDesktop && (
            <div className="aviso-desktop" role="status">
              <p>
                A conversa continua no Claude Desktop. O documento ao lado se atualiza sozinho a
                cada registro.
              </p>
              <button type="button" onClick={voltarAConversa}>
                Voltar à conversa aqui
              </button>
            </div>
          )}
          {!conversa && !noDesktop && (
            <div className="iniciar-conversa">
              <p>Como continuar esta análise?</p>
              {vista?.sessao && (
                <button type="button" onClick={retomar}>
                  Retomar a conversa
                </button>
              )}{" "}
              <button
                type="button"
                disabled={!vista}
                onClick={() => vista && conversar({ promptInicial: vista.promptRetomada })}
              >
                Nova conversa sobre esta análise
              </button>
            </div>
          )}
          {conversa && (
            // Escondido (e não desmontado) no Desktop: desmontar encerraria o processo.
            <div hidden={noDesktop} className="painel-terminal">
              <div className="terminal-controles">
                <PromptsProntos
                  api={api}
                  sistema={sistema}
                  analise={analise}
                  habilitado={vivo}
                  aoEnviar={enviar}
                />
                <button type="button" disabled={!vivo} onClick={continuarNoDesktop}>
                  Continuar no Claude Desktop
                </button>
              </div>
              {falhaEnvio && (
                <p className="suave" role="alert">
                  {falhaEnvio}
                </p>
              )}
              <TerminalClaude
                key={conversa.chave}
                ref={controle}
                sistema={sistema}
                pedido={conversa.pedido}
                aoMudarEstado={setEstadoTerminal}
                api={apiTerminal}
                criarTela={criarTela}
              />
            </div>
          )}
        </section>
        <section className="painel-documento" aria-label="Documento ao vivo">
          {trecho && <TrechoEvidencia carga={trecho} aoFechar={() => setTrecho(null)} />}
          {erro && (
            <p className="trecho-erro" role="alert">
              {erro}
            </p>
          )}
          {!vista && !erro && <p className="suave">Carregando a análise…</p>}
          {vista && (
            <DocumentoAoVivo
              vista={vista}
              evidenciaAberta={trecho?.chave ?? null}
              aoAbrirEvidencia={abrirEvidencia}
            />
          )}
        </section>
      </div>
    </div>
  );
}
