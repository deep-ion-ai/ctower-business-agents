import { useCallback, useEffect, useRef, useState, type CSSProperties } from "react";
import {
  apiAlteracoesTauri,
  intervaloDe,
  mensagemDe,
  type ApiAlteracoes,
  type ApiAnalise,
  type Evidencia,
  type Pergunta,
} from "./analise";
import type { Revisao } from "./AcoesRevisao";
import { AprovacaoAnalise } from "./AprovacaoAnalise";
import { AvisoAlteracao } from "./AvisoAlteracao";
import { BotaoPainel } from "./BotaoPainel";
import { ClientesConversa, type Modo } from "./ClientesConversa";
import { DivisorColunas, useLarguraConversa } from "./DivisorColunas";
import {
  apiConversaTauri,
  comoOpcoes,
  principal,
  type ApiConversa,
  type ClienteConversa,
  type OpcoesConversa,
} from "./conversa";
import {
  chaveEvidencia,
  DocumentoAoVivo,
  ROTULO_ANALISE,
  SeloSessao,
  type OrigemEvidencia,
} from "./DocumentoAoVivo";
import { ExportacaoAnalise } from "./ExportacaoAnalise";
import { PromptsProntos } from "./PromptsProntos";
import { apiRevisaoTauri, type ApiRevisao, type DecisaoRevisao } from "./revisao";
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

/** Painel aberto na barra (um por vez). */
type Menu = "alteracao" | "continuar" | "aprovar" | "exportar";

const ONDE_TEXTO: Record<ClienteConversa, string> = {
  terminalEmbutido: "aqui, no terminal do app",
  claudeDesktop: "no Claude Desktop",
  terminalDoAnalista: "no seu terminal",
};

interface Props {
  api: ApiAnalise;
  projeto: string;
  analise: string;
  abertura: Abertura;
  aoVoltar: () => void;
  /** Para testes. */
  apiTerminal?: ApiTerminal;
  apiConversa?: ApiConversa;
  /** Aviso de alteração na pasta do projeto (#29); para testes. */
  apiAlteracoes?: ApiAlteracoes;
  /** Revisão, aprovação e exportação (#37); para testes. */
  apiRevisao?: ApiRevisao;
  criarTela?: () => TelaTerminal;
}

const AVISO_DESKTOP =
  "A conversa continua no Claude Desktop. O documento ao lado se atualiza sozinho a cada registro.";
const AVISO_LINK_DESKTOP =
  "Abrimos o Claude Desktop com o pedido: confirme a pasta e envie. O documento ao lado se atualiza sozinho a cada registro.";
const AVISO_APROFUNDAR =
  "Aprofundamento registrado. Com a conversa aberta aqui, o pedido vai direto ao agente; noutro cliente, peça para ele aprofundar a análise.";
const AVISO_TERMINAL =
  "A conversa continua no seu terminal. O documento ao lado se atualiza sozinho a cada registro.";

/**
 * Tela da análise (spec, seções 4.2, 4.7, 6.3, 6.4 e 10; #130): o documento montado
 * ao vivo à esquerda, com a maior largura e o trecho da evidência logo abaixo da
 * afirmação, e a conversa à direita. Em telas estreitas, o documento vem primeiro.
 *
 * A barra do topo traz o id, o título e o estado da análise e, à direita, o aviso de
 * alteração (ícone com contador), o menu "continuar a conversa" e os painéis de
 * aprovação e exportação.
 *
 * A conversa acontece num de três clientes, o preferido do primeiro uso primeiro: o
 * terminal embutido, o Claude Desktop (link com o pedido, `/desktop` com a sessão viva
 * ou `claude --desktop --resume`) e o terminal do analista (comando para copiar ou
 * link `claude-cli://`). Fora do terminal embutido, a coluna da conversa mostra só o
 * aviso curto e o botão para voltar à conversa aqui; o terminal fica montado,
 * escondido.
 */
export function PainelAnalise({
  api,
  projeto,
  analise,
  abertura,
  aoVoltar,
  apiTerminal,
  apiConversa = apiConversaTauri,
  criarTela,
  apiAlteracoes = apiAlteracoesTauri,
  apiRevisao = apiRevisaoTauri,
}: Props) {
  const { vista, erro, recarregar } = useAnaliseAoVivo(api, projeto, analise);
  const [revisando, setRevisando] = useState(false);
  const colunas = useRef<HTMLDivElement>(null);
  const [larguraConversa, mudarLarguraConversa] = useLarguraConversa();
  const [falhaRevisao, setFalhaRevisao] = useState<string | null>(null);
  const [opcoes, setOpcoes] = useState<OpcoesConversa | null>(null);
  const [opcoesProntas, setOpcoesProntas] = useState(false);
  const [conversa, setConversa] = useState<{ chave: number; pedido: PedidoClaude } | null>(
    null,
  );
  const [fora, setFora] = useState<Fora | null>(null);
  const [estadoTerminal, setEstadoTerminal] = useState<EstadoTerminal | null>(null);
  const [trecho, setTrecho] = useState<CargaTrecho | null>(null);
  const [origemTrecho, setOrigemTrecho] = useState<OrigemEvidencia | null>(null);
  const [menu, setMenu] = useState<Menu | null>(null);
  const [falhaEnvio, setFalhaEnvio] = useState<string | null>(null);
  const [aviso, setAviso] = useState<string | null>(null);
  const [ocupado, setOcupado] = useState(false);
  const controle = useRef<ControleTerminal>(null);
  const comecou = useRef(false);
  const vivo = estadoTerminal?.fase === "aberto";
  // As opções trazem a sessão só se a conversa dela foi gravada (dá para retomar);
  // sem as opções (comando ausente), vale a sessão vista pelos hooks.
  const sessao = opcoes ? opcoes.sessao : (vista?.sessao?.id ?? null);
  const onde: ClienteConversa | null = fora
    ? fora.cliente
    : conversa
      ? "terminalEmbutido"
      : null;

  const fecharMenu = useCallback(() => setMenu(null), []);
  const alternar = (m: Menu) => () => setMenu((atual) => (atual === m ? null : m));

  // As opções dependem da sessão ligada: relidas quando ela muda.
  useEffect(() => {
    let cancelado = false;
    // Sem o comando (ou com resposta inesperada), fica só o terminal embutido.
    Promise.resolve()
      .then(() => apiConversa.opcoes(projeto, analise))
      .then((o) => !cancelado && setOpcoes(comoOpcoes(o)))
      .catch(() => !cancelado && setOpcoes(null))
      .finally(() => !cancelado && setOpcoesProntas(true));
    return () => {
      cancelado = true;
    };
  }, [apiConversa, projeto, analise, vista?.sessao?.id]);

  const conversar = useCallback((pedido: PedidoClaude) => {
    setFora(null);
    setAviso(null);
    setConversa((c) => ({ chave: (c?.chave ?? 0) + 1, pedido }));
  }, []);

  /**
   * Retomada desta análise: a sessão dela pelo identificador ou, sem sessão conhecida,
   * uma conversa nova com o pedido de retomada. Nunca `--continue`, que abre a última
   * conversa da pasta, de qualquer análise (#130).
   */
  const pedidoRetomada = useCallback(
    (): PedidoClaude =>
      sessao
        ? { retomada: { modo: "sessao", id: sessao } }
        : { retomada: { modo: "nova" }, promptInicial: opcoes?.pedido || undefined },
    [sessao, opcoes?.pedido],
  );

  const retomar = useCallback(() => {
    conversar(pedidoRetomada());
  }, [conversar, pedidoRetomada]);

  const falhar = (e: unknown) => setFalhaEnvio(mensagemDe(e));

  const abrirPorLink = (cliente: "claudeDesktop" | "terminalDoAnalista") => {
    setFalhaEnvio(null);
    apiConversa
      .abrirPorLink(projeto, analise, cliente)
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
        apiConversa.passouAoDesktop(projeto, analise).catch(() => undefined);
      })
      .catch(falhar);
  };

  const noDesktop = (modo: Modo) => {
    setMenu(null);
    setFalhaEnvio(null);
    setAviso(null);
    if (modo === "nova") return abrirPorLink("claudeDesktop");
    if (vivo) return continuarNoDesktop();
    setOcupado(true);
    apiConversa
      .retomarNoDesktop(projeto, analise)
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
    setMenu(null);
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
    setMenu(null);
    if (modo === "retomar") retomar();
    else if (vista) conversar({ promptInicial: vista.promptRetomada });
  };

  const voltarAConversa = () => {
    setMenu(null);
    if (vivo) {
      setFora(null);
      setAviso(null);
    } else retomar();
  };

  /** Pedido de aprofundamento: com a sessão viva aqui, o prompt pronto vai ao terminal. */
  const aprofundar = (numero: number, pedido?: string) => {
    if (!vivo) return setAviso(AVISO_APROFUNDAR);
    api
      .promptsProntos(projeto, analise)
      .then((lista) => {
        const pronto = lista.find((p) => p.id === "aprofundar");
        // O terminal envia a cada quebra de linha: o pedido vai numa linha só.
        const detalhe = pedido?.replace(/\s+/g, " ").trim();
        if (pronto)
          enviar(
            `${pronto.texto} Comece pela afirmação ${numero}` +
              (detalhe ? `; o analista quer: ${detalhe}` : "."),
          );
        else setAviso(AVISO_APROFUNDAR);
      })
      .catch(() => setAviso(AVISO_APROFUNDAR));
  };

  /**
   * Envia ao agente as respostas ainda não enviadas, numa mensagem só, e as marca
   * como enviadas.
   */
  const enviarRespostas = (perguntas: Pergunta[]) => {
    const linha = (t: string) => t.replace(/\s+/g, " ").trim();
    const respostas = perguntas
      .map((p) =>
        p.estado === "respondida"
          ? `Pergunta ${p.numero} ("${linha(p.texto)}"): ${linha(p.resposta)}`
          : "",
      )
      .filter(Boolean)
      .join(" | ");
    const texto =
      `Respondi perguntas da análise ${analise}. ${respostas}. ` +
      "Use estas respostas para atualizar a análise pelo ctower: registre com `afirmar` o " +
      "que elas confirmam ou corrigem e, se surgir uma dúvida nova, use `perguntar`.";
    setFalhaEnvio(null);
    const numeros = perguntas.map((p) => p.numero);
    controle.current
      ?.enviarPrompt(texto)
      .then(
        // A recusa da marcação aparece como falha da revisão.
        () => revisao.aoDecidir({ tipo: "respostas-enviadas", numeros }).catch(() => undefined),
        falhar,
      );
  };

  const revisao: Revisao = {
    ocupado: revisando,
    respostas: { disponivel: vivo, aoEnviar: enviarRespostas },
    aoDecidir: (d: DecisaoRevisao) => {
      setRevisando(true);
      setFalhaRevisao(null);
      return apiRevisao
        .revisar(projeto, analise, d)
        .then(() => {
          recarregar();
          if (d.tipo === "afirmacao" && d.estado === "aprofundar") aprofundar(d.numero, d.pedido);
        })
        .catch((e: unknown) => {
          setFalhaRevisao(mensagemDe(e));
          throw e;
        })
        .finally(() => setRevisando(false));
    },
  };

  const fecharTrecho = () => {
    setTrecho(null);
    setOrigemTrecho(null);
  };

  const abrirEvidencia = (e: Evidencia, origem: OrigemEvidencia) => {
    const chave = chaveEvidencia(e);
    // A mesma evidência no mesmo lugar: fecha o trecho.
    if (trecho?.chave === chave && origemTrecho === origem) return fecharTrecho();
    setOrigemTrecho(origem);
    const { inicio, fim } = intervaloDe(e.linhas);
    setTrecho({ tipo: "carregando", chave });
    api
      .trechoEvidencia(projeto, e.arquivo, inicio, fim)
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
      <header className="painel-barra">
        <button className="btn" type="button" onClick={aoVoltar}>
          ← Análises
        </button>
        <div className="painel-identidade">
          <div className="painel-titulo-linha">
            <h2
              className="painel-titulo"
              aria-label={vista ? `${analise}: ${vista.analise.titulo}` : analise}
            >
              <span className="painel-id">{analise}</span>
              {vista && <span>{vista.analise.titulo}</span>}
            </h2>
            {vista && (
              <span
                className="badge"
                data-tone={vista.analise.estado === "aprovada" ? "done" : undefined}
              >
                {ROTULO_ANALISE[vista.analise.estado]}
              </span>
            )}
          </div>
          {vista && (
            <div className="painel-subtitulo muted">
              <span>versão {vista.analise.versao}</span>
              <SeloSessao sessao={vista.sessao} />
            </div>
          )}
        </div>
        <div className="painel-acoes">
          <AvisoAlteracao
            api={apiAlteracoes}
            vigiar={api.vigiar}
            projeto={projeto}
            analise={analise}
            aberto={menu === "alteracao"}
            aoAlternar={alternar("alteracao")}
            aoFechar={fecharMenu}
          />
          <BotaoPainel
            aberto={menu === "continuar"}
            aoAlternar={alternar("continuar")}
            aoFechar={fecharMenu}
            rotulo="Continuar a conversa"
            classeBotao="btn-icone btn-continuar"
            conteudo={<IconeConversa />}
            rotuloPainel="Onde continuar a conversa"
            classePainel="menu-continuar"
          >
            <p className="muted">
              {onde
                ? `A conversa está ${ONDE_TEXTO[onde]}.`
                : "Nenhuma conversa aberta. Escolha onde continuar esta análise."}
            </p>
            <ClientesConversa
              opcoes={opcoes}
              temSessao={sessao !== null}
              onde={onde}
              vivo={vivo}
              ocupado={ocupado}
              podeNova={!!vista}
              aoAqui={aqui}
              aoVoltarAqui={voltarAConversa}
              aoDesktop={noDesktop}
              aoMeuTerminal={noMeuTerminal}
              aoLinkTerminal={linkTerminal}
              aoCopiar={copiar}
            />
          </BotaoPainel>
          {vista && (
            <>
              <span className="separador" aria-hidden="true" />
              <BotaoPainel
                aberto={menu === "aprovar"}
                aoAlternar={alternar("aprovar")}
                aoFechar={fecharMenu}
                rotulo="Aprovar"
                rotuloPainel="Aprovar a análise"
                classePainel="painel-revisao"
              >
                <AprovacaoAnalise
                  api={apiRevisao}
                  projeto={projeto}
                  analise={analise}
                  versao={vista.analise.versao}
                  aoAprovar={recarregar}
                />
              </BotaoPainel>
              <BotaoPainel
                aberto={menu === "exportar"}
                aoAlternar={alternar("exportar")}
                aoFechar={fecharMenu}
                rotulo="Exportar"
                rotuloPainel="Exportar o documento"
                classePainel="painel-revisao"
              >
                <ExportacaoAnalise
                  api={apiRevisao}
                  projeto={projeto}
                  analise={analise}
                  aprovada={vista.analise.estado === "aprovada"}
                />
              </BotaoPainel>
            </>
          )}
        </div>
      </header>
      <div
        className="painel-colunas"
        ref={colunas}
        style={{ "--largura-conversa": `${larguraConversa}px` } as CSSProperties}
      >
        <section className="painel-documento" aria-label="Documento ao vivo">
          {erro && (
            <p className="error-text" role="alert">
              {erro}
            </p>
          )}
          {!vista && !erro && <p className="muted">Carregando a análise…</p>}
          {falhaRevisao && (
            <p className="error-text" role="alert">
              {falhaRevisao}
            </p>
          )}
          {vista && (
            <DocumentoAoVivo
              vista={vista}
              semCabecalho
              evidenciaAberta={trecho?.chave ?? null}
              aoAbrirEvidencia={abrirEvidencia}
              origemTrecho={origemTrecho}
              trecho={trecho && <TrechoEvidencia carga={trecho} aoFechar={fecharTrecho} />}
              revisao={revisao}
            />
          )}
        </section>
        <DivisorColunas
          colunas={colunas}
          largura={larguraConversa}
          aoMudar={mudarLarguraConversa}
        />
        <section className="painel-conversa" aria-label="Conversa">
          <div className="conversa-cabecalho">
            <span className="ponto-conversa" data-onde={onde ?? "nenhum"} aria-hidden="true" />
            <span className="conversa-onde">
              Conversa · {onde ? ONDE_TEXTO[onde] : "nenhuma aberta"}
            </span>
            {conversa && !fora && (
              <PromptsProntos
                api={api}
                projeto={projeto}
                analise={analise}
                habilitado={vivo}
                aoEnviar={enviar}
              />
            )}
          </div>
          {falhaEnvio && (
            <p className="error-text" role="alert">
              {falhaEnvio}
            </p>
          )}
          {aviso && (
            <p className="muted" role="status">
              {aviso}
            </p>
          )}
          {fora && (
            <div className="aviso-desktop" role="status">
              <p>{fora.aviso}</p>
              <button className="btn" type="button" onClick={voltarAConversa}>
                Voltar à conversa aqui
              </button>
            </div>
          )}
          {!conversa && !fora && (abertura.tipo === "reabrir" || opcoesProntas) && (
            <div className="sem-conversa">
              <p className="muted">Nenhuma conversa aberta nesta análise.</p>
              <button className="btn" type="button" onClick={() => setMenu("continuar")}>
                Escolher onde continuar
              </button>
            </div>
          )}
          {conversa && (
            // Escondido (e não desmontado) fora daqui: desmontar encerraria o processo.
            <div hidden={fora !== null} className="painel-terminal">
              <TerminalClaude
                key={conversa.chave}
                ref={controle}
                projeto={projeto}
                pedido={conversa.pedido}
                continuacao={pedidoRetomada}
                aoMudarEstado={setEstadoTerminal}
                api={apiTerminal}
                criarTela={criarTela}
              />
            </div>
          )}
        </section>
      </div>
    </div>
  );
}

function IconeConversa() {
  return (
    <>
      <svg
        width="18"
        height="18"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        strokeWidth="2"
        strokeLinecap="round"
        strokeLinejoin="round"
        aria-hidden="true"
      >
        <path d="M21 12a8 8 0 0 1-11.6 7.1L4 20l1-4.6A8 8 0 1 1 21 12z" />
        <path d="M10 9l3 3-3 3" />
      </svg>
      <svg
        width="12"
        height="12"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        strokeWidth="2"
        strokeLinecap="round"
        strokeLinejoin="round"
        aria-hidden="true"
      >
        <path d="M6 9l6 6 6-6" />
      </svg>
    </>
  );
}
