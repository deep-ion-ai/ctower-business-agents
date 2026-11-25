import { ProgressoWorkflow } from "./ProgressoWorkflow";
import { openUrl } from "@tauri-apps/plugin-opener";
import { useCallback, useEffect, useRef, useState, type CSSProperties } from "react";
import {
  apiAlteracoesTauri,
  intervaloDe,
  mensagemDe,
  type ApiAlteracoes,
  type ApiAnalise,
  type EstadoSessao,
  type Evidencia,
  type LinkForja,
  type Pergunta,
} from "./analise";
import type { Revisao } from "./AcoesRevisao";
import { AprovacaoAnalise, useProntidao } from "./AprovacaoAnalise";
import { AprovacaoDialogo } from "./AprovacaoDialogo";
import { FaixaCodigoAlterado } from "./FaixaCodigoAlterado";
import { AprovacaoAta } from "./AprovacaoAta";
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
  evidenciasDaOrigem,
  ROTULO_ANALISE,
  SeloSessao,
  type OrigemEvidencia,
} from "./DocumentoAoVivo";
import { ExportacaoAnalise } from "./ExportacaoAnalise";
import { NovaPrototipo } from "./NovaPrototipo";
import { apiPrototipoTauri, ORIGENS_PROTOTIPO, type ApiPrototipo } from "./prototipo";
import { ParecerAoVivo } from "./ParecerAoVivo";
import { ConversaAgente, type SituacaoAgente } from "./ConversaAgente";
import { PainelLateral, type Aba } from "./PainelLateral";
import { useTerminalVisivel } from "./preferenciaTerminal";
import { PromptsProntos } from "./PromptsProntos";
import { apiPublicacaoTauri, type ApiPublicacao } from "./publicacao";
import { PublicacaoDocs } from "./PublicacaoDocs";
import { contarPendencias } from "./resumoRevisao";
import { apiRevisaoTauri, type ApiRevisao, type DecisaoRevisao, type Pendencia } from "./revisao";
import { AtaAoVivo } from "./AtaAoVivo";
import { apiReuniaoTauri, type ApiReuniao } from "./reunioes";
import { TerminalClaude, type ControleTerminal, type EstadoTerminal } from "./TerminalClaude";
import { TrechoEvidencia, type CargaTrecho } from "./TrechoEvidencia";
import type { CodigoAlterado } from "./FaixaCodigoAlterado";
import type { ApiTerminal, PedidoClaude } from "./terminal";
import type { TelaTerminal } from "./telaXterm";
import { useAnaliseAoVivo } from "./useAnaliseAoVivo";

/** Como a tela chegou à análise. */
export type Abertura =
  /** Análise recém-criada: a conversa começa com o pedido de `iniciar_analise`. */
  | { tipo: "nova"; promptInicial: string }
  /** Análise existente: o analista escolhe retomar a conversa ou começar outra. */
  | { tipo: "reabrir" };

/**
 * A conversa do terminal embutido, para quem fecha a análise: sem processo vivo,
 * viva à espera do analista ou com o agente trabalhando (#212).
 */
export type EstadoConversa = "nenhuma" | "aberta" | "trabalhando";

/** Onde a conversa está, quando não está no terminal embutido. */
type Fora = { cliente: "claudeDesktop" | "terminalDoAnalista"; aviso: string };

/** Painel aberto na barra (um por vez). */
type Menu = "alteracao" | "continuar" | "exportar" | "publicar";

const ONDE_TEXTO: Record<ClienteConversa, string> = {
  terminalEmbutido: "aqui, no terminal do app",
  claudeDesktop: "no Claude Desktop",
  terminalDoAnalista: "no seu terminal",
};

/** Nome curto do lugar da conversa, no seletor do painel lateral. */
const ONDE_CURTO: Record<ClienteConversa, string> = {
  terminalEmbutido: "Aqui no app",
  claudeDesktop: "No Claude Desktop",
  terminalDoAnalista: "No seu terminal",
};

const FALLBACK_APROFUNDAR = "Aprofunde a análise pelo ctower.";
const PEDIDO_OUTRO_EXEMPLO =
  "Dê outro exemplo concreto da regra, com valores diferentes dos que já usou, e registre-o na análise pelo ctower.";
const PEDIDO_SEM_TERMOS =
  "Explique o que a análise diz sem termos técnicos, em linguagem de negócio, como para quem não conhece o sistema.";

/** O que o agente está fazendo, em português, para o cabeçalho da conversa. */
function situacaoDoAgente(
  fora: Fora | null,
  temConversa: boolean,
  terminal: EstadoTerminal | null,
  sessao: EstadoSessao | undefined,
): SituacaoAgente {
  if (fora)
    return {
      texto: `Conversa ${ONDE_TEXTO[fora.cliente]}`,
      tom: "running",
    };
  if (!temConversa) return { texto: "Nenhuma conversa aberta", tom: "muted" };
  switch (terminal?.fase) {
    case undefined:
    case "abrindo":
      return { texto: "Abrindo o Claude Code…", tom: "running" };
    case "erro":
      return { texto: "O Claude Code não abriu", tom: "attention" };
    case "encerrado":
      return { texto: "A sessão terminou", tom: "muted" };
    case "aberto":
      break;
  }
  switch (sessao) {
    case "ativa":
      return { texto: "Trabalhando na análise", tom: "running" };
    case "aguardando-analista":
      return { texto: "Terminou e espera você", tom: "attention" };
    case "concluida":
      return { texto: "Respondeu e espera a sua próxima mensagem", tom: "done" };
    default:
      return { texto: "Pronto para conversar", tom: "done" };
  }
}

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
  /** Ata ao vivo e participantes da análise de uma reunião (#141); para testes. */
  apiReuniao?: ApiReuniao;
  /** Publicação dos documentos aprovados no repositório (#276); para testes. */
  apiPublicacao?: ApiPublicacao;
  /** "Prototipar telas" a partir da análise aprovada (#379); para testes. */
  apiPrototipo?: ApiPrototipo;
  criarTela?: () => TelaTerminal;
  /** Abre outra análise (a criada por um próximo passo da ata, #142). */
  aoAbrirAnalise?: (id: string, abertura: Abertura) => void;
  /** Abre um link no navegador do sistema (o da evidência na forja); para testes. */
  abrirUrl?: (url: string) => Promise<void>;
  /** Avisa quando a conversa do terminal embutido abre, trabalha ou termina. */
  aoMudarConversa?: (estado: EstadoConversa) => void;
}

const AVISO_DESKTOP =
  "A conversa continua no Claude Desktop. O documento ao lado se atualiza sozinho a cada registro.";
const AVISO_LINK_DESKTOP =
  "Abrimos o Claude Desktop com o pedido: confirme a pasta e envie. O documento ao lado se atualiza sozinho a cada registro.";
const AVISO_APROFUNDAR =
  "Aprofundamento registrado. Com a conversa aberta aqui, o pedido vai direto ao agente; noutro cliente, peça para ele aprofundar a análise.";
const AVISO_REVALIDAR =
  "Sem conversa aberta aqui, o pedido não foi enviado: peça ao agente, no cliente em que a conversa está, para revalidar as afirmações marcadas.";
const AVISO_TERMINAL =
  "A conversa continua no seu terminal. O documento ao lado se atualiza sozinho a cada registro.";

/**
 * Tela da análise (spec, seções 4.2, 4.7, 6.3, 6.4 e 10; #130): o documento montado
 * ao vivo à esquerda, com a maior largura e o trecho da evidência logo abaixo da
 * afirmação, e a conversa à direita. Em telas estreitas, o documento vem primeiro.
 *
 * A barra do topo traz o id, o título e o estado da análise e, à direita, o aviso de
 * alteração (ícone com contador), o menu "continuar a conversa" e os painéis de
 * aprovação, exportação e publicação da documentação aprovada no repositório (#276).
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
  apiReuniao = apiReuniaoTauri,
  apiPublicacao = apiPublicacaoTauri,
  apiPrototipo = apiPrototipoTauri,
  aoAbrirAnalise,
  abrirUrl = openUrl,
  aoMudarConversa,
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
  const [linkForja, setLinkForja] = useState<{ chave: string; link: LinkForja | null } | null>(
    null,
  );
  const chaveAberta = useRef<string | null>(null);
  const [aba, setAba] = useState<Aba>("conversa");
  const [terminalVisivel, alternarTerminal] = useTerminalVisivel();
  const [menu, setMenu] = useState<Menu | null>(null);
  const [aprovando, setAprovando] = useState(false);
  const [prototipando, setPrototipando] = useState(false);
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

  // A análise de uma reunião é aprovada pela ata: a prontidão das análises não vale.
  const { prontidao } = useProntidao(
    apiRevisao,
    projeto,
    analise,
    vista?.analise.versao ?? 0,
    !!vista && vista.analise.fluxo !== "reuniao",
  );

  // Quem fecha a análise (a aba da navegação) pergunta antes de encerrar a conversa.
  const estadoConversa: EstadoConversa = !vivo
    ? "nenhuma"
    : vista?.sessao?.estado === "ativa"
      ? "trabalhando"
      : "aberta";
  const avisarConversa = useRef(aoMudarConversa);
  avisarConversa.current = aoMudarConversa;
  useEffect(() => {
    avisarConversa.current?.(estadoConversa);
  }, [estadoConversa]);
  useEffect(() => () => avisarConversa.current?.("nenhuma"), []);

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
  // Interrompida no meio do workflow, a sessão retomada pede o workflow de novo pelo
  // `runId`: o que terminou volta do cache, sem registro duplicado (#371).
  const pedidoNaSessao = opcoes?.pedidoNaSessao ?? vista?.promptNaSessao ?? undefined;
  const pedidoRetomada = useCallback(
    (): PedidoClaude =>
      sessao
        ? { retomada: { modo: "sessao", id: sessao }, promptInicial: pedidoNaSessao || undefined }
        : { retomada: { modo: "nova" }, promptInicial: opcoes?.pedido || undefined },
    [sessao, opcoes?.pedido, pedidoNaSessao],
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

  /** "Continuar sem workflow" (#371): o roteiro completo, na conversa aberta ou numa retomada. */
  const continuarSemWorkflow = (pedido: string) => {
    if (vivo) return enviar(pedido);
    conversar(
      sessao
        ? { retomada: { modo: "sessao", id: sessao }, promptInicial: pedido }
        : { retomada: { modo: "nova" }, promptInicial: pedido },
    );
  };

  /** Mensagem da caixa do painel: a mesma entrada de digitar no terminal e apertar Enter. */
  const escreverAoAgente = (texto: string): Promise<void> => {
    setFalhaEnvio(null);
    const terminal = controle.current;
    if (!terminal) return Promise.reject(new Error("o terminal não está aberto"));
    return terminal.enviarPrompt(texto).catch((e: unknown) => {
      falhar(e);
      throw e;
    });
  };

  /** "Aprofundar as pendentes": o prompt pronto de aprofundar, apontado às pendentes. */
  const aprofundarPendentes = () => {
    const pedir = (base: string) =>
      enviar(`${base} Comece pelas afirmações que ainda esperam a revisão do analista.`);
    api
      .promptsProntos(projeto, analise)
      .then((lista) => pedir(lista.find((p) => p.id === "aprofundar")?.texto ?? FALLBACK_APROFUNDAR))
      .catch(() => pedir(FALLBACK_APROFUNDAR));
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
    // O `/desktop` encerra o CLI, e o workflow em segundo plano iria junto (#371).
    const bloqueio = opcoes?.bloqueioDesktop ?? vista?.bloqueioDesktop;
    if (modo === "retomar" && bloqueio) return setFalhaEnvio(bloqueio);
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
    chaveAberta.current = null;
    setTrecho(null);
    setOrigemTrecho(null);
    setLinkForja(null);
    setAba("conversa");
  };

  /** Abre o trecho na aba Código: o chip do documento só escolhe a evidência. */
  const abrirEvidencia = (e: Evidencia, origem: OrigemEvidencia) => {
    const chave = chaveEvidencia(e);
    chaveAberta.current = chave;
    setOrigemTrecho(origem);
    setAba("codigo");
    const { inicio, fim } = intervaloDe(e.linhas);
    setTrecho({ tipo: "carregando", chave });
    setLinkForja(null);
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
    // O link é um complemento: sem repositório cadastrado, a aba fica sem o botão.
    Promise.resolve()
      .then(() => api.linkForja(projeto, e.arquivo, inicio, fim, e.commit))
      .then((link) => {
        if (chaveAberta.current === chave) setLinkForja({ chave, link });
      })
      .catch(() => undefined);
  };

  // Evidência anterior e próxima: as da mesma afirmação (ou sugestão, ou diagrama).
  const evidenciasDaAfirmacao =
    vista && origemTrecho ? evidenciasDaOrigem(vista.analise, origemTrecho) : [];
  const indiceEvidencia = evidenciasDaAfirmacao.findIndex((e) => chaveEvidencia(e) === trecho?.chave);
  const irParaEvidencia = (passo: number) => {
    const alvo = evidenciasDaAfirmacao[indiceEvidencia + passo];
    if (alvo && origemTrecho) abrirEvidencia(alvo, origemTrecho);
  };

  /** Leva ao cartão da afirmação: rola e foca (depois de o diálogo devolver o foco). */
  const irParaAfirmacao = (numero: number) => {
    setAprovando(false);
    setMenu(null);
    setTimeout(() => {
      const el = document.querySelector<HTMLElement>(`[data-testid="afirmacao-${numero}"]`);
      el?.scrollIntoView?.({ block: "center" });
      el?.focus();
    }, 0);
  };

  const revisarPendentes = (pendencias: Pendencia[]) => {
    const primeira = pendencias.find((p) => p.tipo === "afirmacao" && p.numero !== null);
    if (primeira?.numero != null) irParaAfirmacao(primeira.numero);
    else setAprovando(false);
  };

  /** Pede ao agente que revalide as afirmações do código alterado; `false` sem conversa viva. */
  const pedirRevalidacao = (alterados: CodigoAlterado[]) => {
    if (!vivo) {
      setAviso(AVISO_REVALIDAR);
      return false;
    }
    const numeros = [...new Set(alterados.flatMap((c) => c.afirmacoes))].sort((a, b) => a - b);
    enviar(
      `O código citado na análise ${analise} mudou (${alterados.map((c) => c.arquivo).join(", ")}). ` +
        `Revalide as afirmações ${numeros.join(", ")}: confira cada evidência no código atual e, ` +
        "pelo ctower, registre com `afirmar` o que continua valendo ou o que mudou.",
    );
    return true;
  };

  const aprovada = vista?.analise.estado === "aprovada";
  // A análise aprovada de onde as regras vêm pode virar protótipo de telas (#379).
  const prototipavel =
    aprovada && !!vista && ORIGENS_PROTOTIPO.has(vista.analise.fluxo) && !!aoAbrirAnalise;
  const deAta = vista?.analise.fluxo === "reuniao";
  // A mesma conta da faixa e do diálogo de aprovação (#222).
  const pendencias = !deAta && !aprovada && prontidao ? prontidao.pendencias : null;
  const pendentes = pendencias ? contarPendencias(pendencias) : null;
  const agenteTrabalhando = !aprovada && vista?.sessao?.estado === "ativa";
  const pronta = pendentes === 0 && !!prontidao?.podeAprovar;
  const rotuloAprovar = aprovada
    ? "Análise aprovada"
    : agenteTrabalhando
      ? "Aguardando o agente"
      : pronta
        ? "Aprovar análise"
        : "Aprovar…";
  const selo = aprovada
    ? null
    : agenteTrabalhando
      ? "Agente trabalhando"
      : pronta
        ? "Pronta para aprovar"
        : pendentes
          ? `Aguardando sua revisão · ${pendentes} ${pendentes === 1 ? "pendente" : "pendentes"}`
          : null;
  const tituloAprovar =
    deAta
      ? "Aprovar a ata"
      : vista?.analise.fluxo === "revisar"
        ? "Aprovar o parecer"
        : vista?.analise.fluxo === "historias"
          ? "Aprovar as histórias"
          : vista?.analise.fluxo === "cenarios"
            ? "Aprovar os cenários"
            : "Aprovar a análise";

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
                title={ROTULO_ANALISE[vista.analise.estado]}
                data-tone={vista.analise.estado === "aprovada" ? "done" : undefined}
              >
                {ROTULO_ANALISE[vista.analise.estado]}
              </span>
            )}
            {vista && selo && (
              <span
                className="badge selo-aprovacao"
                role="status"
                aria-label="Situação da aprovação"
                title={selo}
                data-tone={pronta ? "done" : agenteTrabalhando ? "running" : "attention"}
              >
                {selo}
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
          {vista && (
            <>
              <span className="separador" aria-hidden="true" />
              <button
                className="btn btn-primary"
                type="button"
                aria-haspopup="dialog"
                disabled={agenteTrabalhando}
                onClick={() => {
                  setMenu(null);
                  setAprovando(true);
                }}
              >
                {rotuloAprovar}
              </button>
              {prototipavel && (
                <button
                  className="btn"
                  type="button"
                  aria-haspopup="dialog"
                  onClick={() => {
                    setMenu(null);
                    setPrototipando(true);
                  }}
                >
                  Prototipar telas
                </button>
              )}
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
                  csv={vista.analise.fluxo === "cenarios"}
                />
              </BotaoPainel>
              <BotaoPainel
                aberto={menu === "publicar"}
                aoAlternar={alternar("publicar")}
                aoFechar={fecharMenu}
                rotulo="Publicar"
                titulo="Publicar os documentos aprovados do projeto no repositório"
                rotuloPainel="Publicar a documentação no repositório"
                classePainel="painel-revisao painel-publicacao"
              >
                <PublicacaoDocs api={apiPublicacao} projeto={projeto} abrirUrl={abrirUrl} />
              </BotaoPainel>
            </>
          )}
        </div>
      </header>
      {vista && prototipando && aoAbrirAnalise && (
        <AprovacaoDialogo titulo="Prototipar telas" aoFechar={() => setPrototipando(false)}>
          <NovaPrototipo
            api={api}
            apiPrototipo={apiPrototipo}
            projetos={[{ id: projeto, nome: projeto }]}
            origemFixa={{ id: analise, titulo: vista.analise.titulo }}
            semTitulo
            aoCancelar={() => setPrototipando(false)}
            aoCriar={(_p, criada) => {
              setPrototipando(false);
              aoAbrirAnalise(criada.analise.id, {
                tipo: "nova",
                promptInicial: criada.promptInicial,
              });
            }}
          />
        </AprovacaoDialogo>
      )}
      {vista && aprovando && (
        <AprovacaoDialogo titulo={tituloAprovar} aoFechar={() => setAprovando(false)}>
          {deAta ? (
            // A análise de uma reunião é aprovada com a ata, que apaga o transcript.
            <AprovacaoAta
              api={apiReuniao}
              projeto={projeto}
              analise={analise}
              versao={vista.analise.versao}
              aoAprovar={recarregar}
            />
          ) : (
            <AprovacaoAnalise
              api={apiRevisao}
              projeto={projeto}
              analise={analise}
              versao={vista.analise.versao}
              aoAprovar={recarregar}
              aoIrPara={(p) => p.numero !== null && irParaAfirmacao(p.numero)}
              aoRevisarPendentes={revisarPendentes}
            />
          )}
        </AprovacaoDialogo>
      )}
      <div
        className="painel-colunas"
        ref={colunas}
        data-sem-conversa={!conversa && !fora && trecho === null ? "" : undefined}
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
            <FaixaCodigoAlterado
              afirmacoes={vista.analise.afirmacoes}
              aoVer={(e, origem) => {
                abrirEvidencia(e, origem);
                irParaAfirmacao(Number(origem.replace("afirmacao-", "")));
              }}
              aoPedir={pedirRevalidacao}
              aoIrPara={irParaAfirmacao}
            />
          )}
          {vista && (
            <ProgressoWorkflow
              analise={vista.analise}
              pedidoSemWorkflow={vista.promptSemWorkflow}
              aoContinuarSemWorkflow={continuarSemWorkflow}
            />
          )}
          {vista?.analise.fluxo === "reuniao" && vista.analise.reuniao && (
            <AtaAoVivo
              api={apiReuniao}
              vigiar={api.vigiar}
              projeto={projeto}
              reuniao={vista.analise.reuniao}
              aoCriarAnalise={
                aoAbrirAnalise &&
                ((c) =>
                  aoAbrirAnalise(c.analise.id, { tipo: "nova", promptInicial: c.promptInicial }))
              }
            />
          )}
          {vista?.analise.fluxo === "revisar" && (
            <ParecerAoVivo
              analise={vista.analise}
              ocupado={revisando}
              aoDecidir={revisao.aoDecidir}
            />
          )}
          {vista && (
            <DocumentoAoVivo
              vista={vista}
              semCabecalho
              evidenciaAberta={trecho?.chave ?? null}
              aoAbrirEvidencia={abrirEvidencia}
              aoFecharEvidencia={fecharTrecho}
              origemTrecho={origemTrecho}
              revisao={revisao}
              pendencias={pendencias}
            />
          )}
        </section>
        <DivisorColunas
          colunas={colunas}
          largura={larguraConversa}
          aoMudar={mudarLarguraConversa}
        />
        <PainelLateral
          aba={aba}
          aoMudarAba={setAba}
          codigoAberto={trecho !== null}
          conversa={
            <ConversaAgente
              situacao={situacaoDoAgente(fora, !!conversa, estadoTerminal, vista?.sessao?.estado)}
              onde={onde}
              seletor={
                <BotaoPainel
                  aberto={menu === "continuar"}
                  aoAlternar={alternar("continuar")}
                  aoFechar={fecharMenu}
                  rotulo="Continuar a conversa"
                  conteudo={`${onde ? ONDE_CURTO[onde] : "Conversa"} ▾`}
                  classeBotao="btn btn-sm seletor-onde"
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
                    bloqueioDesktop={opcoes?.bloqueioDesktop ?? vista?.bloqueioDesktop ?? null}
                    aoAqui={aqui}
                    aoVoltarAqui={voltarAConversa}
                    aoDesktop={noDesktop}
                    aoMeuTerminal={noMeuTerminal}
                    aoLinkTerminal={linkTerminal}
                    aoCopiar={copiar}
                  />
                </BotaoPainel>
              }
              mostraAcoes={!!conversa && !fora}
              habilitado={vivo}
              acoesRapidas={[
                {
                  id: "aprofundar-pendentes",
                  rotulo: "Aprofundar as pendentes",
                  aoClicar: aprofundarPendentes,
                },
                {
                  id: "outro-exemplo",
                  rotulo: "Outro exemplo",
                  aoClicar: () => enviar(PEDIDO_OUTRO_EXEMPLO),
                },
                {
                  id: "sem-termos-tecnicos",
                  rotulo: "Explicar sem termos técnicos",
                  aoClicar: () => enviar(PEDIDO_SEM_TERMOS),
                },
              ]}
              maisPedidos={
                <PromptsProntos
                  api={api}
                  projeto={projeto}
                  analise={analise}
                  habilitado={vivo}
                  aoEnviar={enviar}
                  excluir={["aprofundar"]}
                />
              }
              terminalVisivel={terminalVisivel}
              aoAlternarTerminal={alternarTerminal}
              aoEscrever={escreverAoAgente}
            >
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
                // Escondido (e não desmontado) fora daqui ou pela preferência: desmontar
                // encerraria o processo, e reabrir mostra a saída completa.
                <div hidden={fora !== null || !terminalVisivel} className="painel-terminal">
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
            </ConversaAgente>
          }
          codigo={
            trecho ? (
              <TrechoEvidencia
                carga={trecho}
                aoFechar={fecharTrecho}
                posicao={
                  indiceEvidencia >= 0
                    ? { indice: indiceEvidencia, total: evidenciasDaAfirmacao.length }
                    : undefined
                }
                aoAnterior={() => irParaEvidencia(-1)}
                aoProxima={() => irParaEvidencia(1)}
                link={linkForja?.chave === trecho.chave ? linkForja.link : null}
                aoAbrirLink={(url) => {
                  setFalhaEnvio(null);
                  abrirUrl(url).catch(() => setFalhaEnvio("Não foi possível abrir o link."));
                }}
              />
            ) : (
              <p className="muted sem-evidencia">
                Nenhuma evidência aberta. Escolha uma evidência no documento para ver o código
                citado, com as linhas em destaque.
              </p>
            )
          }
        />
      </div>
    </div>
  );
}
