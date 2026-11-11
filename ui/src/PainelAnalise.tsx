import { useCallback, useEffect, useRef, useState } from "react";
import { intervaloDe, mensagemDe, type ApiAnalise, type Evidencia } from "./analise";
import { ClientesConversa, type Modo } from "./ClientesConversa";
import {
  apiConversaTauri,
  comoOpcoes,
  desktopDisponivel,
  principal,
  type ApiConversa,
  type OpcoesConversa,
} from "./conversa";
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

/** Onde a conversa está, quando não está no terminal embutido. */
type Fora = { cliente: "claudeDesktop" | "terminalDoAnalista"; aviso: string };

interface Props {
  api: ApiAnalise;
  sistema: string;
  analise: string;
  abertura: Abertura;
  aoVoltar: () => void;
  /** Para testes. */
  apiTerminal?: ApiTerminal;
  apiConversa?: ApiConversa;
  criarTela?: () => TelaTerminal;
}

const AVISO_DESKTOP =
  "A conversa continua no Claude Desktop. O documento ao lado se atualiza sozinho a cada registro.";
const AVISO_LINK_DESKTOP =
  "Abrimos o Claude Desktop com o pedido: confirme a pasta e envie. O documento ao lado se atualiza sozinho a cada registro.";
const AVISO_TERMINAL =
  "A conversa continua no seu terminal. O documento ao lado se atualiza sozinho a cada registro.";

/**
 * Tela da análise: a conversa de um lado e, do outro, o documento montado ao vivo,
 * com o trecho da evidência escolhida (spec, seções 4.2, 4.7, 6.3, 6.4 e 10).
 *
 * A conversa acontece num de três clientes, o preferido do primeiro uso primeiro: o
 * terminal embutido, o Claude Desktop (link com o pedido, `/desktop` com a sessão viva
 * ou `claude --desktop --resume`) e o terminal do analista (comando para copiar ou
 * link `claude-cli://`). Fora do terminal embutido, fica o documento, com o botão
 * para voltar à conversa aqui.
 */
export function PainelAnalise({
  api,
  sistema,
  analise,
  abertura,
  aoVoltar,
  apiTerminal,
  apiConversa = apiConversaTauri,
  criarTela,
}: Props) {
  const { vista, erro } = useAnaliseAoVivo(api, sistema, analise);
  const [opcoes, setOpcoes] = useState<OpcoesConversa | null>(null);
  const [opcoesProntas, setOpcoesProntas] = useState(false);
  const [conversa, setConversa] = useState<{ chave: number; pedido: PedidoClaude } | null>(
    null,
  );
  const [fora, setFora] = useState<Fora | null>(null);
  const [estadoTerminal, setEstadoTerminal] = useState<EstadoTerminal | null>(null);
  const [trecho, setTrecho] = useState<CargaTrecho | null>(null);
  const [falhaEnvio, setFalhaEnvio] = useState<string | null>(null);
  const [aviso, setAviso] = useState<string | null>(null);
  const [ocupado, setOcupado] = useState(false);
  const controle = useRef<ControleTerminal>(null);
  const comecou = useRef(false);
  const vivo = estadoTerminal?.fase === "aberto";
  const sessao = vista?.sessao?.id ?? opcoes?.sessao ?? null;

  // As opções dependem da sessão ligada: relidas quando ela muda.
  useEffect(() => {
    let cancelado = false;
    // Sem o comando (ou com resposta inesperada), fica só o terminal embutido.
    Promise.resolve()
      .then(() => apiConversa.opcoes(sistema, analise))
      .then((o) => !cancelado && setOpcoes(comoOpcoes(o)))
      .catch(() => !cancelado && setOpcoes(null))
      .finally(() => !cancelado && setOpcoesProntas(true));
    return () => {
      cancelado = true;
    };
  }, [apiConversa, sistema, analise, vista?.sessao?.id]);

  const conversar = useCallback((pedido: PedidoClaude) => {
    setFora(null);
    setAviso(null);
    setConversa((c) => ({ chave: (c?.chave ?? 0) + 1, pedido }));
  }, []);

  const retomar = useCallback(() => {
    conversar({ retomada: sessao ? { modo: "sessao", id: sessao } : { modo: "continuar" } });
  }, [conversar, sessao]);

  const falhar = (e: unknown) => setFalhaEnvio(mensagemDe(e));

  const abrirPorLink = (cliente: "claudeDesktop" | "terminalDoAnalista") => {
    setFalhaEnvio(null);
    apiConversa
      .abrirPorLink(sistema, analise, cliente)
      .then(() =>
        setFora({
          cliente,
          aviso: cliente === "claudeDesktop" ? AVISO_LINK_DESKTOP : AVISO_TERMINAL,
        }),
      )
      .catch(falhar);
  };

  // Análise nova: a conversa começa no cliente preferido, assim que as opções chegam.
  useEffect(() => {
    if (abertura.tipo !== "nova" || !opcoesProntas || comecou.current) return;
    comecou.current = true;
    if (principal(opcoes) === "claudeDesktop") abrirPorLink("claudeDesktop");
    else conversar({ promptInicial: abertura.promptInicial });
    // Uma vez só, com as primeiras opções.
  }, [opcoesProntas]);

  const enviar = (texto: string) => {
    setFalhaEnvio(null);
    controle.current?.enviarPrompt(texto).catch(falhar);
  };

  /** `/desktop`: o Claude Code salva a sessão, abre-a no Desktop e sai. */
  const continuarNoDesktop = () => {
    setFalhaEnvio(null);
    controle.current
      ?.continuarNoDesktop()
      .then(() => {
        setFora({ cliente: "claudeDesktop", aviso: AVISO_DESKTOP });
        apiConversa.passouAoDesktop(sistema, analise).catch(() => undefined);
      })
      .catch(falhar);
  };

  const noDesktop = (modo: Modo) => {
    setFalhaEnvio(null);
    setAviso(null);
    if (modo === "nova") return abrirPorLink("claudeDesktop");
    if (vivo) return continuarNoDesktop();
    setOcupado(true);
    apiConversa
      .retomarNoDesktop(sistema, analise)
      .then((r) => {
        if (r.aberto) setFora({ cliente: "claudeDesktop", aviso: AVISO_DESKTOP });
        else
          setFalhaEnvio(
            `O Claude Desktop não abriu a sessão${r.mensagem ? `: ${r.mensagem}` : "."} ` +
              "Feche a conversa nas outras janelas e tente de novo, ou comece uma conversa nova no Desktop, que retoma a análise pelo ctower.",
          );
      })
      .catch((e: unknown) => {
        const m = typeof e === "object" && e !== null && "mensagem" in e ? e.mensagem : e;
        falhar(m);
      })
      .finally(() => setOcupado(false));
  };

  const linkTerminal = () => {
    // Duas cópias do Claude na mesma conversa gravam uma por cima da outra.
    if (vivo) controle.current?.encerrar().catch(() => undefined);
    abrirPorLink("terminalDoAnalista");
  };

  const noMeuTerminal = () => {
    setFalhaEnvio(null);
    if (vivo) controle.current?.encerrar().catch(() => undefined);
    setFora({ cliente: "terminalDoAnalista", aviso: AVISO_TERMINAL });
  };

  const copiar = (texto: string) => {
    setFalhaEnvio(null);
    apiConversa
      .copiar(texto)
      .then(() => setAviso("Comando copiado. Cole no seu terminal."))
      .catch(() => setFalhaEnvio("Não foi possível copiar: selecione o comando e copie."));
  };

  const aqui = (modo: Modo) => {
    if (modo === "retomar") retomar();
    else if (vista) conversar({ promptInicial: vista.promptRetomada });
  };

  const voltarAConversa = () => {
    if (vivo) {
      setFora(null);
      setAviso(null);
    } else retomar();
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

  const clientes = (semAqui: boolean) => (
    <ClientesConversa
      opcoes={opcoes}
      temSessao={sessao !== null}
      semAqui={semAqui}
      ocupado={ocupado}
      podeNova={!!vista}
      aoAqui={aqui}
      aoDesktop={noDesktop}
      aoLinkTerminal={linkTerminal}
      aoCopiar={copiar}
    />
  );

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
          {falhaEnvio && (
            <p className="trecho-erro" role="alert">
              {falhaEnvio}
            </p>
          )}
          {aviso && (
            <p className="suave" role="status">
              {aviso}
            </p>
          )}
          {fora && (
            <div className="aviso-desktop" role="status">
              <p>{fora.aviso}</p>
              <button type="button" onClick={voltarAConversa}>
                Voltar à conversa aqui
              </button>
              {clientes(true)}
            </div>
          )}
          {!conversa && !fora && (abertura.tipo === "reabrir" || opcoesProntas) && (
            <div className="iniciar-conversa">
              <p>Como continuar esta análise?</p>
              {clientes(false)}
            </div>
          )}
          {conversa && (
            // Escondido (e não desmontado) fora daqui: desmontar encerraria o processo.
            <div hidden={fora !== null} className="painel-terminal">
              <div className="terminal-controles">
                <PromptsProntos
                  api={api}
                  sistema={sistema}
                  analise={analise}
                  habilitado={vivo}
                  aoEnviar={enviar}
                />
                {desktopDisponivel(opcoes) && (
                  <button type="button" disabled={!vivo} onClick={continuarNoDesktop}>
                    Continuar no Claude Desktop
                  </button>
                )}{" "}
                <button type="button" onClick={noMeuTerminal}>
                  Continuar no meu terminal
                </button>
              </div>
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
