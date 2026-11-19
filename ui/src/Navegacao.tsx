import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type KeyboardEvent as ReactKeyboardEvent,
  type PointerEvent as ReactPointerEvent,
  type ReactNode,
} from "react";
import { mensagemDe, type ApiAnalise } from "./analise";
import type { AbrirLink } from "./ClientesIA";
import {
  apiConhecimentoTauri,
  type AnaliseInicio,
  type ApiConhecimento,
  type Inicio,
} from "./conhecimento";
import { DialogoConfirmacao } from "./DialogoConfirmacao";
import { ROTULO_SESSAO } from "./DocumentoAoVivo";
import type { Abertura, EstadoConversa } from "./PainelAnalise";
import { PainelDemanda } from "./PainelDemanda";
import { Icon, type IconName } from "./shell/icons";
import { Titlebar } from "./shell/Titlebar";
import { TelaAnalise } from "./TelaAnalise";
import type { ApiConfiguracoes } from "./configuracoes";
import { TelaBase } from "./TelaBase";
import { TelaConfiguracoes } from "./TelaConfiguracoes";
import { TelaInicio } from "./TelaInicio";
import { TelaMapa } from "./TelaMapa";
import { TelaProjeto } from "./TelaProjeto";
import type { ApiTerminal, ProjetoResumo } from "./terminal";
import type { TelaTerminal } from "./telaXterm";

/** Tela visível e o que ela abre. */
export type Tela =
  | { tipo: "inicio" }
  | { tipo: "projeto"; projeto?: string }
  | { tipo: "base"; documento?: string }
  | {
      tipo: "analise";
      projeto?: string;
      pergunta?: string;
      aberta?: { id: string; abertura: Abertura };
    }
  | { tipo: "configuracoes" }
  | { tipo: "demanda"; projeto: string; demanda: string }
  /** Mapa de funcionalidades do projeto (#208): fora das análises. */
  | { tipo: "mapa"; projeto?: string };

interface Props {
  /** Tela ao abrir (ex.: a análise sugerida no fim do primeiro uso). */
  inicial?: Tela;
  api?: ApiConhecimento;
  /** Para testes. */
  apiAnalise?: ApiAnalise;
  /** Para testes. */
  apiConfiguracoes?: ApiConfiguracoes;
  /** Para testes: o terminal embutido das análises. */
  apiTerminal?: ApiTerminal;
  /** Para testes. */
  criarTela?: () => TelaTerminal;
  abrirLink?: AbrirLink;
  /** Abre o assistente do primeiro uso. */
  aoPrimeiroUso?: () => void;
  /** Mostra "Refazer o primeiro uso" no pé do painel lateral. */
  refazerPrimeiroUso?: () => void;
  produto?: string;
  versao?: string | null;
  /** Avisos do app acima das abas (ex.: falha ao ler a configuração). */
  banner?: ReactNode;
}

const AREAS: { tipo: Tela["tipo"]; rotulo: string; icone: IconName }[] = [
  { tipo: "inicio", rotulo: "Início", icone: "home" },
  { tipo: "projeto", rotulo: "Projeto", icone: "project" },
  { tipo: "base", rotulo: "Base de conhecimento", icone: "book" },
  { tipo: "analise", rotulo: "Análises", icone: "analysis" },
  { tipo: "mapa", rotulo: "Mapa de funcionalidades", icone: "features" },
  { tipo: "configuracoes", rotulo: "Configurações", icone: "settings" },
];

// A demanda não tem área própria na barra de atividades: abre pela tela do projeto.
const ROTULO_AREA = {
  ...Object.fromEntries(AREAS.map((a) => [a.tipo, a.rotulo])),
  demanda: "Demanda",
} as Record<Tela["tipo"], string>;
const ICONE_AREA = {
  ...Object.fromEntries(AREAS.map((a) => [a.tipo, a.icone])),
  demanda: "file",
} as Record<Tela["tipo"], IconName>;

/** Aba aberta na área de editor: a tela que ela mostra e o contador de remontagem. */
interface Aba {
  chave: string;
  tela: Tela;
  /** Muda a cada navegação para a aba, para a tela abrir do zero com o pedido. */
  vez: number;
}

/** Uma aba por tela e por objeto aberto (projeto, documento ou análise). */
export function chaveDaTela(t: Tela): string {
  switch (t.tipo) {
    case "projeto":
      return t.projeto ? `projeto:${t.projeto}` : "projeto";
    case "base":
      return t.documento ? `doc:${t.documento}` : "base";
    case "analise":
      return t.aberta ? `analise:${t.projeto ?? ""}:${t.aberta.id}` : "analise";
    case "demanda":
      return `demanda:${t.projeto}:${t.demanda}`;
    default:
      return t.tipo;
  }
}

/**
 * Aba que fica montada (escondida) quando outra está à vista: a de uma análise
 * aberta e a do mapa de funcionalidades, cujo terminal roda o agente. Desmontar
 * encerraria o processo (#212).
 */
const ficaViva = (a: Aba) =>
  (a.tela.tipo === "analise" && !!a.tela.aberta) || a.tela.tipo === "mapa";

const nomeDoArquivo = (caminho: string) =>
  (caminho.split("/").pop() ?? caminho).replace(/\.md$/, "");

/** Largura do painel lateral (mesmos limites do Control Tower). */
const SIDE_MIN = 170;
const SIDE_MAX = 600;
const SIDE_KEY = "cba.sideWidth";
const clampWidth = (w: number) => Math.min(SIDE_MAX, Math.max(SIDE_MIN, Math.round(w)));

/** De quanto em quanto tempo o explorador relê as análises. */
const INTERVALO_EXPLORADOR = 5000;

const ESTREITA = "(max-width: 900px)";
const telaEstreita = () => typeof matchMedia === "function" && matchMedia(ESTREITA).matches;

function larguraGravada(): number {
  try {
    const w = Number(localStorage.getItem(SIDE_KEY));
    return w ? clampWidth(w) : 260;
  } catch {
    return 260;
  }
}

/**
 * As análises do explorador, sem repetir e em ordem estável: por projeto e número.
 * Reordenar por atividade faria o clique no lugar de antes abrir outra análise (#227).
 */
export function analisesDoExplorador(inicio: Inicio): AnaliseInicio[] {
  const vistas = new Map<string, AnaliseInicio>();
  for (const a of [...inicio.emAndamento, ...inicio.recentes]) {
    vistas.set(`${a.projeto}/${a.id}`, a);
  }
  return [...vistas.values()].sort(
    (a, b) =>
      a.projetoNome.localeCompare(b.projetoNome, "pt-BR") ||
      a.projeto.localeCompare(b.projeto) ||
      a.id.localeCompare(b.id, "pt-BR", { numeric: true }),
  );
}

/** Tom do ponto da análise: âmbar só quando a sessão espera o analista. */
function tomDaAnalise(a: AnaliseInicio): string | undefined {
  if (a.sessao?.estado === "aguardando-analista") return "attention";
  if (a.sessao?.estado === "ativa") return "running";
  if (a.estado === "aprovada") return "done";
  return undefined;
}

/**
 * Navegação depois do primeiro uso (spec, seção 10), no shell do Control Tower
 * (#85): barra de título, barra de atividades com as áreas (início, projeto, base,
 * análises e configurações), painel lateral com os projetos, as análises e os
 * documentos abertos, área de editor com abas e barra de status. Cada tela pede às
 * outras o que abrir (um projeto, um documento, uma análise) por `navegar`, que
 * abre ou foca a aba do objeto. Abaixo de 900 px o painel lateral vira gaveta.
 */
export function Navegacao({
  inicial = { tipo: "inicio" },
  api = apiConhecimentoTauri,
  apiAnalise,
  apiConfiguracoes,
  apiTerminal,
  criarTela,
  abrirLink,
  aoPrimeiroUso,
  refazerPrimeiroUso,
  produto = "Ctower Business Agents",
  versao = null,
  banner,
}: Props) {
  const [abas, setAbas] = useState<Aba[]>(() => [
    { chave: chaveDaTela(inicial), tela: inicial, vez: 0 },
  ]);
  const [ativa, setAtiva] = useState<string>(() => chaveDaTela(inicial));
  const [projetos, setProjetos] = useState<ProjetoResumo[]>([]);
  const [inicio, setInicio] = useState<Inicio | null>(null);
  const [documentos, setDocumentos] = useState<string[]>(
    inicial.tipo === "base" && inicial.documento ? [inicial.documento] : [],
  );
  const [falha, setFalha] = useState<string | null>(null);
  // Muda a cada navegação e quando as configurações mexem nos projetos ou na base.
  const [recarga, setRecarga] = useState(0);
  const [ladoAberto, setLadoAberto] = useState(() => !telaEstreita());
  const [largura, setLargura] = useState(larguraGravada);
  const contador = useRef(0);

  const aba = abas.find((a) => a.chave === ativa) ?? null;

  // Com muitas abas a faixa rola: a aba ativa sempre fica à vista.
  const faixaAbas = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const ativaEl = faixaAbas.current?.querySelector<HTMLElement>('.etab[data-on="true"]');
    ativaEl?.scrollIntoView?.({ block: "nearest", inline: "nearest" });
  }, [ativa, abas.length]);
  const tela = aba?.tela ?? null;

  /** Documentos abertos nesta sessão, o mais recente primeiro (painel lateral). */
  const lembrarDocumento = useCallback((doc: string) => {
    setDocumentos((ds) => (ds[0] === doc ? ds : [doc, ...ds.filter((d) => d !== doc)].slice(0, 20)));
  }, []);

  // Alterações sem salvar no editor de textos (das configurações): sair da aba, ou
  // fechá-la, pede confirmação. `saida` guarda a ação à espera da resposta.
  const pendenteRef = useRef(false);
  const [saida, setSaida] = useState<(() => void) | null>(null);
  const aoPendencia = useCallback((p: boolean) => {
    pendenteRef.current = p;
  }, []);
  const sairDaAba = useCallback((acao: () => void) => {
    if (pendenteRef.current) setSaida(() => acao);
    else acao();
  }, []);

  // Conversa de cada aba de análise aberta: fechar a aba com ela viva pede confirmação.
  const conversas = useRef(new Map<string, EstadoConversa>());
  const [fechamento, setFechamento] = useState<{ chave: string; trabalhando: boolean; mapa: boolean } | null>(
    null,
  );
  const [fechamentoOutras, setFechamentoOutras] = useState<{
    manter: string;
    vivas: string[];
    trabalhando: boolean;
  } | null>(null);
  const aoMudarConversa = useCallback((chave: string, estado: EstadoConversa) => {
    if (estado === "nenhuma") conversas.current.delete(chave);
    else conversas.current.set(chave, estado);
  }, []);

  const navegarAgora = useCallback((t: Tela) => {
    const chave = chaveDaTela(t);
    const vez = ++contador.current;
    setAbas((atuais) => {
      const existente = atuais.find((a) => a.chave === chave);
      if (!existente) return [...atuais, { chave, tela: t, vez }];
      // A análise já aberta só ganha o foco: abrir do zero encerraria a conversa (#212).
      if (ficaViva(existente)) return atuais;
      return atuais.map((a) => (a.chave === chave ? { chave, tela: t, vez } : a));
    });
    setAtiva(chave);
    if (t.tipo === "base" && t.documento) lembrarDocumento(t.documento);
    setRecarga((r) => r + 1);
    // Na gaveta, escolher um item fecha o painel.
    if (telaEstreita()) setLadoAberto(false);
  }, [lembrarDocumento]);

  const navegar = useCallback((t: Tela) => sairDaAba(() => navegarAgora(t)), [sairDaAba, navegarAgora]);

  useEffect(() => {
    let ativo = true;
    api
      .situacao()
      .then((s) => {
        if (!ativo) return;
        setProjetos(s.projetos);
        setFalha(null);
      })
      .catch((e: unknown) => ativo && setFalha(mensagemDe(e)));
    api
      .inicio()
      .then((i) => ativo && setInicio(i))
      // O início mostra a própria falha; o painel lateral só fica sem as listas.
      .catch(() => ativo && setInicio(null));
    return () => {
      ativo = false;
    };
  }, [api, recarga]);

  // O explorador acompanha o que acontece fora das telas (análise criada, agente que
  // passou a esperar o analista, base editada): relê o início quando a base muda e,
  // por garantia, a cada poucos segundos (#227).
  useEffect(() => {
    let ativo = true;
    const ler = () =>
      api
        .inicio()
        .then((i) => ativo && setInicio(i))
        .catch(() => undefined);
    const relogio = setInterval(ler, INTERVALO_EXPLORADOR);
    let parar: (() => void) | null = null;
    api
      .vigiarBase(ler)
      .then((p) => (ativo ? (parar = p) : p()))
      .catch(() => undefined);
    return () => {
      ativo = false;
      clearInterval(relogio);
      parar?.();
    };
  }, [api]);

  /**
   * Barra de atividades: vai à última aba da área (ou abre uma); na área atual,
   * mostra ou esconde o painel lateral.
   */
  const irParaArea = (tipo: Tela["tipo"]) => {
    if (tela?.tipo === tipo) {
      setLadoAberto((v) => !v);
      return;
    }
    const existente = [...abas].reverse().find((a) => a.tela.tipo === tipo);
    if (existente) ativar(existente.chave);
    else navegar({ tipo } as Tela);
  };

  const ativar = (chave: string) => {
    if (chave === ativa) return;
    sairDaAba(() => {
      setAtiva(chave);
      if (telaEstreita()) setLadoAberto(false);
    });
  };

  const fechar = (chave: string) => {
    const conversa = conversas.current.get(chave);
    if (conversa) {
      setFechamento({
        chave,
        trabalhando: conversa === "trabalhando",
        mapa: abas.find((a) => a.chave === chave)?.tela.tipo === "mapa",
      });
      return;
    }
    if (chave === ativa && pendenteRef.current) {
      sairDaAba(() => fecharAgora(chave));
      return;
    }
    fecharAgora(chave);
  };

  /** Fecha todas as abas menos `manter`, respeitando as conversas vivas (#212, #227). */
  const fecharOutras = (manter: string) => {
    const outras = abas.filter((a) => a.chave !== manter);
    if (outras.length === 0) return;
    const vivas = outras.filter((a) => conversas.current.has(a.chave));
    if (vivas.length > 0) {
      setFechamentoOutras({
        manter,
        vivas: vivas.map((a) => a.chave),
        trabalhando: vivas.some((a) => conversas.current.get(a.chave) === "trabalhando"),
      });
      return;
    }
    fecharVarias(manter, outras.map((a) => a.chave));
  };

  const fecharVarias = (manter: string, chaves: string[]) => {
    for (const c of chaves) conversas.current.delete(c);
    const resto = abas.filter((a) => !chaves.includes(a.chave));
    setAbas(resto);
    if (chaves.includes(ativa)) setAtiva(resto.some((a) => a.chave === manter) ? manter : (resto[0]?.chave ?? ""));
  };

  const fecharAgora = (chave: string) => {
    conversas.current.delete(chave);
    const i = abas.findIndex((a) => a.chave === chave);
    const resto = abas.filter((a) => a.chave !== chave);
    setAbas(resto);
    if (chave === ativa) setAtiva(resto[Math.min(i, resto.length - 1)]?.chave ?? "");
  };

  const mudarLargura = (w: number) => {
    const c = clampWidth(w);
    setLargura(c);
    try {
      localStorage.setItem(SIDE_KEY, String(c));
    } catch {
      // Sem localStorage: vale só nesta abertura.
    }
  };

  const nomeProjeto = (id: string) =>
    projetos.find((s) => s.id === id)?.nome ??
    inicio?.projetos.find((s) => s.id === id)?.nome ??
    id;

  const tituloDaAba = (t: Tela): string => {
    if (t.tipo === "projeto" && t.projeto) return nomeProjeto(t.projeto);
    if (t.tipo === "base" && t.documento) return nomeDoArquivo(t.documento);
    if (t.tipo === "analise" && t.aberta) return t.aberta.id;
    if (t.tipo === "demanda") return t.demanda;
    return ROTULO_AREA[t.tipo];
  };

  const analises = inicio ? analisesDoExplorador(inicio) : [];
  const aRevalidar = inicio?.projetos.reduce((n, s) => n + s.aRevalidar, 0) ?? 0;
  const aguardando = analises.filter((a) => a.sessao?.estado === "aguardando-analista").length;

  return (
    <div className="shell">
      <Titlebar
        produto={produto}
        versao={versao}
        antes={
          <button
            type="button"
            className="ibtn"
            aria-label={ladoAberto ? "Fechar painel lateral" : "Abrir painel lateral"}
            aria-expanded={ladoAberto}
            aria-controls="painel-lateral"
            onClick={() => setLadoAberto((v) => !v)}
          >
            <Icon name="sidebar" />
          </button>
        }
      />

      <div className="shell-work">
        <nav className="activitybar" aria-label="Telas">
          {AREAS.map((a) => (
            <button
              key={a.tipo}
              type="button"
              className="ab-item"
              aria-label={a.rotulo}
              data-tip={a.rotulo}
              aria-current={tela?.tipo === a.tipo ? "page" : undefined}
              onClick={() => irParaArea(a.tipo)}
            >
              <Icon name={a.icone} size={22} />
            </button>
          ))}
        </nav>

        <div
          id="painel-lateral"
          className="sidepanel"
          role="region"
          aria-label="Painel lateral"
          hidden={!ladoAberto}
          style={{ width: largura }}
        >
          <div className="sp">
            <div className="sp-head">
              <h2 className="sp-title">Explorador</h2>
            </div>
            <div className="sp-body">
              <GrupoLateral titulo="Projetos" quantos={inicio?.projetos.length ?? projetos.length}>
                {(inicio?.projetos ?? projetos.map((s) => ({ ...s, aRevalidar: 0, erro: null }))).map(
                  (s) => (
                    <LinhaLateral
                      key={s.id}
                      icone="project"
                      rotulo={s.nome}
                      sub={
                        s.erro
                          ? "com erro"
                          : s.aRevalidar > 0
                            ? `${s.aRevalidar} a revalidar`
                            : undefined
                      }
                      tom={s.erro ? "error" : s.aRevalidar > 0 ? "attention" : undefined}
                      selecionada={ativa === `projeto:${s.id}`}
                      onClick={() => navegar({ tipo: "projeto", projeto: s.id })}
                    />
                  ),
                )}
              </GrupoLateral>
              <GrupoLateral titulo="Análises" quantos={analises.length}>
                {analises.map((a) => (
                  <LinhaLateral
                    key={`${a.projeto}/${a.id}`}
                    ponto
                    rotulo={`${a.id}: ${a.titulo}`}
                    sub={
                      a.sessao && a.sessao.estado !== "encerrada"
                        ? `${a.projetoNome} · ${ROTULO_SESSAO[a.sessao.estado]}`
                        : a.projetoNome
                    }
                    tom={tomDaAnalise(a)}
                    selecionada={ativa === `analise:${a.projeto}:${a.id}`}
                    onClick={() =>
                      navegar({
                        tipo: "analise",
                        projeto: a.projeto,
                        aberta: { id: a.id, abertura: { tipo: "reabrir" } },
                      })
                    }
                  />
                ))}
              </GrupoLateral>
              <GrupoLateral titulo="Documentos" quantos={documentos.length} vazio="Nenhum documento aberto.">
                {documentos.map((d) => (
                  <LinhaLateral
                    key={d}
                    icone="file"
                    rotulo={nomeDoArquivo(d)}
                    sub={d}
                    selecionada={ativa === `doc:${d}`}
                    onClick={() => navegar({ tipo: "base", documento: d })}
                  />
                ))}
              </GrupoLateral>
            </div>
            {refazerPrimeiroUso && (
              <div className="sp-foot">
                <button
                  type="button"
                  className="link"
                  onClick={() => sairDaAba(refazerPrimeiroUso)}
                >
                  Refazer o primeiro uso
                </button>
              </div>
            )}
          </div>
        </div>
        {ladoAberto && (
          <>
            <Resizer largura={largura} aoMudar={mudarLargura} />
            {/* Gaveta (tela estreita): tocar fora fecha. */}
            <div className="sidepanel-scrim" aria-hidden="true" onClick={() => setLadoAberto(false)} />
          </>
        )}

        <div className="shell-main">
          {banner}
          {falha && tela?.tipo !== "inicio" && (
            <div className="banner" role="alert">
              {falha}
            </div>
          )}
          <div className="etabs-bar">
            <div className="etabs" role="tablist" aria-label="Abas abertas"
              ref={faixaAbas}
              onWheel={(e) => {
                // A roda do mouse rola a faixa de abas, que não tem barra à vista.
                if (faixaAbas.current && Math.abs(e.deltaY) > Math.abs(e.deltaX))
                  faixaAbas.current.scrollLeft += e.deltaY;
              }}
            >
              {abas.map((a) => {
                const titulo = tituloDaAba(a.tela);
                return (
                  <div key={a.chave} className="etab" data-on={a.chave === ativa}>
                    <button
                      type="button"
                      role="tab"
                      className="etab-main"
                      aria-selected={a.chave === ativa}
                      aria-controls="area-editor"
                      title={titulo}
                      onClick={() => ativar(a.chave)}
                    >
                      <Icon name={ICONE_AREA[a.tela.tipo]} size={14} />
                      <span className="etab-title">{titulo}</span>
                    </button>
                    <button
                      type="button"
                      className="ibtn etab-close"
                      aria-label={`Fechar ${titulo}`}
                      onClick={() => fechar(a.chave)}
                    >
                      <Icon name="close" size={14} />
                    </button>
                  </div>
                );
              })}
            </div>
            {abas.length > 1 && aba && (
              <button
                type="button"
                className="etabs-outras"
                title="Fecha todas as abas, menos a que está à vista"
                onClick={() => sairDaAba(() => fecharOutras(ativa))}
              >
                Fechar outras
              </button>
            )}
          </div>
          <div className="editor" role="tabpanel" id="area-editor" aria-label="Área de editor">
            <div className="editor-tela">
              {!aba && (
                <div className="page">
                  <p className="empty">
                    Nenhuma aba aberta. Escolha uma área na barra de atividades ou um item do painel
                    lateral.
                  </p>
                </div>
              )}
              {aba?.tela.tipo === "inicio" && (
                <TelaInicio
                  key={aba.vez}
                  api={api}
                  navegar={navegar}
                  abrirLink={abrirLink}
                  aoPrimeiroUso={aoPrimeiroUso}
                  apiProjeto={apiConfiguracoes}
                  aoMudarProjetos={() => setRecarga((r) => r + 1)}
                />
              )}
              {aba?.tela.tipo === "projeto" && (
                <TelaProjeto
                  key={aba.vez}
                  api={api}
                  projetos={projetos}
                  projetoInicial={aba.tela.projeto}
                  navegar={navegar}
                />
              )}
              {aba?.tela.tipo === "base" && (
                <TelaBase
                  key={aba.vez}
                  api={api}
                  projetos={projetos}
                  documentoInicial={aba.tela.documento}
                  navegar={navegar}
                  aoAbrirDocumento={lembrarDocumento}
                  aoPendencia={aoPendencia}
                />
              )}
              {aba?.tela.tipo === "analise" && !ficaViva(aba) && (
                <TelaAnalise
                  key={aba.vez}
                  api={apiAnalise}
                  apiTerminal={apiTerminal}
                  criarTela={criarTela}
                  projetoInicial={aba.tela.projeto}
                  perguntaInicial={aba.tela.pergunta}
                  aoPrimeiroUso={aoPrimeiroUso}
                  aoAbrirAnalise={(projeto, id, abertura) =>
                    navegar({ tipo: "analise", projeto, aberta: { id, abertura } })
                  }
                />
              )}
              {/* As análises abertas ficam montadas, escondidas fora da vez: o agente
                  continua trabalhando enquanto o analista está noutra aba (#212). */}
              {abas.filter(ficaViva).map((a) =>
                a.tela.tipo === "analise" ? (
                  <div key={a.chave} className="editor-aba" hidden={a.chave !== ativa}>
                    <TelaAnalise
                      key={a.vez}
                      api={apiAnalise}
                      apiTerminal={apiTerminal}
                      criarTela={criarTela}
                      projetoInicial={a.tela.projeto}
                      aberturaInicial={a.tela.aberta}
                      aoPrimeiroUso={aoPrimeiroUso}
                      aoAbrirAnalise={(projeto, id, abertura) =>
                        navegar({ tipo: "analise", projeto, aberta: { id, abertura } })
                      }
                      aoVoltar={(projeto) => navegar({ tipo: "analise", projeto })}
                      aoMudarConversa={(estado) => aoMudarConversa(a.chave, estado)}
                    />
                  </div>
                ) : a.tela.tipo === "mapa" ? (
                  <div key={a.chave} className="editor-aba" hidden={a.chave !== ativa}>
                    <TelaMapa
                      key={a.vez}
                      projetos={projetos}
                      projetoInicial={a.tela.projeto}
                      apiAnalise={apiAnalise}
                      apiTerminal={apiTerminal}
                      criarTela={criarTela}
                      navegar={navegar}
                      aoMudarConversa={(estado) => aoMudarConversa(a.chave, estado)}
                    />
                  </div>
                ) : null,
              )}
              {aba?.tela.tipo === "demanda" && (
                <PainelDemanda
                  key={aba.vez}
                  api={api}
                  projeto={aba.tela.projeto}
                  demanda={aba.tela.demanda}
                  navegar={navegar}
                />
              )}
              {aba?.tela.tipo === "configuracoes" && (
                <TelaConfiguracoes
                  key={aba.vez}
                  api={apiConfiguracoes}
                  aoMudarProjetos={() => setRecarga((r) => r + 1)}
                  aoPendencia={aoPendencia}
                />
              )}
            </div>
          </div>
        </div>
      </div>

      {saida && (
        <DialogoConfirmacao
          titulo="Alterações não salvas"
          aoCancelar={() => setSaida(null)}
          acoes={
            <>
              <button
                type="button"
                className="btn btn-primary"
                autoFocus
                onClick={() => setSaida(null)}
              >
                Continuar editando
              </button>
              <button
                type="button"
                className="btn"
                onClick={() => {
                  const acao = saida;
                  pendenteRef.current = false;
                  setSaida(null);
                  acao();
                }}
              >
                Descartar e sair
              </button>
            </>
          }
        >
          <p>
            Há um editor com alterações que não foram salvas. Sair agora as
            descarta.
          </p>
        </DialogoConfirmacao>
      )}

      {fechamento && (
        <DialogoConfirmacao
          titulo={fechamento.mapa ? "Fechar o mapa" : "Fechar a análise"}
          aoCancelar={() => setFechamento(null)}
          acoes={
            <>
              <button
                type="button"
                className="btn btn-primary"
                autoFocus
                onClick={() => setFechamento(null)}
              >
                Manter a conversa
              </button>
              <button
                type="button"
                className="btn"
                onClick={() => {
                  const { chave } = fechamento;
                  setFechamento(null);
                  fecharAgora(chave);
                }}
              >
                Encerrar a conversa
              </button>
            </>
          }
        >
          <p>
            {fechamento.trabalhando
              ? "O agente ainda está trabalhando. Encerrar a conversa?"
              : fechamento.mapa
                ? "A sessão de mapeamento com o Claude continua aberta. Encerrar a sessão?"
                : "A conversa com o agente continua aberta nesta análise. Encerrar a conversa?"}
          </p>
          <p className="muted">
            {fechamento.mapa
              ? "A sessão de mapeamento com o Claude será encerrada. Uma sessão nova continua o mapa de onde parou."
              : "Fechar a aba encerra o Claude Code desta análise. Dá para retomar a conversa depois, pela própria análise."}
          </p>
        </DialogoConfirmacao>
      )}

      {fechamentoOutras && (
        <DialogoConfirmacao
          titulo="Fechar as outras abas"
          aoCancelar={() => setFechamentoOutras(null)}
          acoes={
            <>
              <button
                type="button"
                className="btn btn-primary"
                autoFocus
                onClick={() => {
                  // Mantém as análises com conversa: fecha só o que não tem processo vivo.
                  const { manter, vivas } = fechamentoOutras;
                  setFechamentoOutras(null);
                  fecharVarias(
                    manter,
                    abas.filter((a) => a.chave !== manter && !vivas.includes(a.chave)).map((a) => a.chave),
                  );
                }}
              >
                Manter as conversas
              </button>
              <button
                type="button"
                className="btn"
                onClick={() => {
                  const { manter } = fechamentoOutras;
                  setFechamentoOutras(null);
                  fecharVarias(manter, abas.filter((a) => a.chave !== manter).map((a) => a.chave));
                }}
              >
                Encerrar as conversas
              </button>
            </>
          }
        >
          <p>
            {fechamentoOutras.vivas.length === 1
              ? "Uma das outras abas tem uma conversa com o agente aberta."
              : `${fechamentoOutras.vivas.length} das outras abas têm uma conversa com o agente aberta.`}
            {fechamentoOutras.trabalhando && " O agente ainda está trabalhando."}
          </p>
          <p className="muted">
            Fechar uma aba encerra o Claude Code dela: nas análises, dá para retomar a conversa
            depois, pela própria análise; no mapa, a sessão de mapeamento será encerrada.
            “Manter as conversas” fecha só as abas sem conversa.
          </p>
        </DialogoConfirmacao>
      )}

      <footer className="statusbar" aria-label="Barra de status">
        <span className="sb-item mono" title={inicio?.base ?? undefined}>
          {inicio?.base ? `Base: ${inicio.base}` : "Base não escolhida"}
        </span>
        <span className="sb-item">
          {projetos.length === 1 ? "1 projeto" : `${projetos.length} projetos`}
        </span>
        <span className="sb-spacer" />
        {aguardando > 0 && (
          <span className="sb-item sb-warn">
            <Icon name="warning" size={12} />
            {aguardando === 1 ? "1 sessão aguarda você" : `${aguardando} sessões aguardam você`}
          </span>
        )}
        {aRevalidar > 0 && (
          <button
            type="button"
            className="sb-item sb-warn"
            onClick={() => {
              const s = inicio?.projetos.find((x) => x.aRevalidar > 0);
              if (s) navegar({ tipo: "projeto", projeto: s.id });
            }}
          >
            <Icon name="warning" size={12} />
            {aRevalidar === 1 ? "1 regra a revalidar" : `${aRevalidar} regras a revalidar`}
          </button>
        )}
      </footer>
    </div>
  );
}

function GrupoLateral({
  titulo,
  quantos,
  vazio = "Nada ainda.",
  children,
}: {
  titulo: string;
  quantos: number;
  vazio?: string;
  children: ReactNode;
}) {
  return (
    <div className="sp-group" role="group" aria-label={titulo}>
      <h3 className="sp-group-title">
        {titulo}
        <span className="sp-group-count">{quantos}</span>
      </h3>
      {quantos === 0 ? <p className="sp-empty">{vazio}</p> : <ul className="sp-rows">{children}</ul>}
    </div>
  );
}

function LinhaLateral({
  rotulo,
  sub,
  icone,
  ponto,
  tom,
  selecionada,
  onClick,
}: {
  rotulo: string;
  sub?: string;
  icone?: IconName;
  ponto?: boolean;
  tom?: string;
  selecionada: boolean;
  onClick: () => void;
}) {
  return (
    <li className={sub ? "sp-row sp-row-2" : "sp-row"} data-selected={selecionada}>
      <button
        type="button"
        className="sp-row-main"
        aria-current={selecionada ? "true" : undefined}
        onClick={onClick}
      >
        {ponto || !icone ? (
          <span className="sp-dot" data-tone={tom} aria-hidden="true" />
        ) : (
          <Icon name={icone} size={14} />
        )}
        <span className="sp-row-text">
          <span className="sp-row-label">{rotulo}</span>
          {sub && (
            <span className="sp-row-sub" data-tone={tom}>
              {sub}
            </span>
          )}
        </span>
      </button>
    </li>
  );
}

/** Divisória arrastável do painel lateral; setas mudam a largura pelo teclado. */
function Resizer({ largura, aoMudar }: { largura: number; aoMudar: (w: number) => void }) {
  const inicio = useRef<{ x: number; w: number } | null>(null);
  const onPointerDown = (e: ReactPointerEvent) => {
    inicio.current = { x: e.clientX, w: largura };
    (e.target as Element).setPointerCapture?.(e.pointerId);
  };
  const onPointerMove = (e: ReactPointerEvent) => {
    if (inicio.current) aoMudar(inicio.current.w + e.clientX - inicio.current.x);
  };
  const parar = () => {
    inicio.current = null;
  };
  const onKeyDown = (e: ReactKeyboardEvent) => {
    const passo = e.shiftKey ? 64 : 16;
    const w =
      e.key === "ArrowLeft"
        ? largura - passo
        : e.key === "ArrowRight"
          ? largura + passo
          : e.key === "Home"
            ? SIDE_MIN
            : e.key === "End"
              ? SIDE_MAX
              : null;
    if (w === null) return;
    e.preventDefault();
    aoMudar(w);
  };
  return (
    <div
      className="sidepanel-resizer"
      role="separator"
      aria-orientation="vertical"
      aria-label="Largura do painel lateral"
      aria-valuenow={largura}
      aria-valuemin={SIDE_MIN}
      aria-valuemax={SIDE_MAX}
      tabIndex={0}
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={parar}
      onPointerCancel={parar}
      onKeyDown={onKeyDown}
    />
  );
}
