import { useCallback, useEffect, useRef, useState, type ReactNode } from "react";
import { apiAnaliseTauri, mensagemDe, type ApiAnalise, type EstadoAnalise } from "./analise";
import { seloAtividades, type AmbienteConferido } from "./atividades";
import type { AbrirLink } from "./ClientesIA";
import { apiConhecimentoTauri, type ApiConhecimento, type Inicio } from "./conhecimento";
import { DialogoConfirmacao } from "./DialogoConfirmacao";
import { MenuLateral } from "./MenuLateral";
import type { Abertura, EstadoConversa } from "./PainelAnalise";
import { PainelDemanda } from "./PainelDemanda";
import {
  contagensDoMenu,
  ITEM_DA_SECAO,
  lembrarProjeto,
  pendenciasNoInicio,
  ROTULO_SECAO,
  ultimoProjeto,
  type Secao,
} from "./secoesProjeto";
import { SeletorProjeto, type ProjetoNoSeletor } from "./SeletorProjeto";
import type { SituacaoNaBase } from "./issuesProjeto";
import { Icon, type IconName } from "./shell/icons";
import { Titlebar } from "./shell/Titlebar";
import type { ResumoProjeto } from "./resumoProjeto";
import { TelaAnalise } from "./TelaAnalise";
import { TelaAtividades } from "./TelaAtividades";
import { apiConfiguracoesTauri, type ApiConfiguracoes } from "./configuracoes";
import { TelaBase } from "./TelaBase";
import { TelaCodigo } from "./TelaCodigo";
import { TelaDemandas } from "./TelaDemandas";
import { TelaConfiguracoes } from "./TelaConfiguracoes";
import { TelaInicio } from "./TelaInicio";
import { TelaMapa } from "./TelaMapa";
import { VincularIssues } from "./VincularIssues";
import { apiMapaTauri, type ApiMapa } from "./funcionalidades";
import { VisaoGeral } from "./VisaoGeral";
import type { ApiTerminal, ProjetoResumo } from "./terminal";
import type { TelaTerminal } from "./telaXterm";

/**
 * O que as telas pedem para abrir. Uma parte é seção (troca no lugar, no projeto da
 * barra de título): início/Atividades, projeto/Visão geral, base sem documento,
 * análises sem análise aberta, demandas, mapa/Funcionalidades, código, configurações
 * e configurar. A outra é objeto e abre numa aba: a análise aberta, o documento da
 * base, a demanda e o vínculo das issues (#319, §11.2 da revisão de UX).
 */
export type Tela =
  /** Atividades (#321), no projeto pedido ou no da barra de título. */
  | { tipo: "inicio"; projeto?: string }
  /** Visão geral do projeto; até a #320, a tela Projeto. */
  | { tipo: "projeto"; projeto?: string }
  | { tipo: "base"; documento?: string }
  | {
      tipo: "analise";
      projeto?: string;
      pergunta?: string;
      aberta?: { id: string; abertura: Abertura };
      /** Abre a lista já filtrada por situação (ex.: "aguardando revisão", da Visão geral). */
      situacao?: EstadoAnalise;
      /** Abre a lista com o painel "Nova análise" aberto. */
      nova?: boolean;
    }
  | { tipo: "configuracoes" }
  /** "Configurar projeto", aberto pelo cabeçalho da Visão geral (#319). */
  | { tipo: "configurar"; projeto?: string }
  | { tipo: "demanda"; projeto: string; demanda: string }
  /** Demandas do projeto (#323): issues do rastreador e demandas à mão; `situacao` abre a lista já filtrada. */
  | { tipo: "demandas"; projeto?: string; situacao?: SituacaoNaBase }
  /** Mapa de funcionalidades do projeto (#208): a seção Funcionalidades. */
  | { tipo: "mapa"; projeto?: string }
  /** Mapa estrutural e "O que mudou no código" (#319). */
  | { tipo: "codigo"; projeto?: string }
  /** Vincular as issues do rastreador às funcionalidades (#270), em aba própria. */
  | { tipo: "vincular"; projeto: string };

interface Props {
  /** Tela ao abrir (ex.: a análise sugerida no fim do primeiro uso). Sem ela, a Visão geral. */
  inicial?: Tela;
  api?: ApiConhecimento;
  /** Para testes. */
  apiAnalise?: ApiAnalise;
  /** Para testes. */
  apiConfiguracoes?: ApiConfiguracoes;
  /** Para testes: o mapa de funcionalidades lido pela Visão geral. */
  apiMapa?: Pick<ApiMapa, "mapa">;
  /** Para testes: o terminal embutido das análises. */
  apiTerminal?: ApiTerminal;
  /** Para testes. */
  criarTela?: () => TelaTerminal;
  abrirLink?: AbrirLink;
  /** Abre o assistente do primeiro uso. */
  aoPrimeiroUso?: () => void;
  /** Mostra "Refazer o primeiro uso" no pé do menu lateral. */
  refazerPrimeiroUso?: () => void;
  produto?: string;
  versao?: string | null;
  /** Avisos do app acima do conteúdo (ex.: falha ao ler a configuração). */
  banner?: ReactNode;
}

/** Ícone da aba de cada objeto. */
const ICONE_OBJETO: Partial<Record<Tela["tipo"], IconName>> = {
  analise: "analysis",
  base: "file",
  demanda: "demand",
  vincular: "jira",
};

/** Aba de um objeto aberto: a tela que ela mostra e o contador de remontagem. */
interface Aba {
  chave: string;
  tela: Tela;
  /** Muda a cada navegação para a aba, para a tela abrir do zero com o pedido. */
  vez: number;
}

/** Seção à vista: o pedido que a abriu (com a pergunta, por exemplo) e a remontagem. */
interface SecaoAberta {
  id: Secao;
  tela: Tela;
  vez: number;
}

/**
 * A aba do objeto que a tela abre (uma por análise, documento, demanda ou vínculo);
 * `null` quando a tela é uma seção, que troca no lugar sem abrir aba.
 */
export function chaveDoObjeto(t: Tela): string | null {
  switch (t.tipo) {
    case "base":
      return t.documento ? `doc:${t.documento}` : null;
    case "analise":
      return t.aberta ? `analise:${t.projeto ?? ""}:${t.aberta.id}` : null;
    case "demanda":
      return `demanda:${t.projeto}:${t.demanda}`;
    case "vincular":
      return `vincular:${t.projeto}`;
    default:
      return null;
  }
}

/** A seção de cada tela; a de um objeto é a seção de onde ele vem. */
export function secaoDaTela(t: Tela): Secao {
  switch (t.tipo) {
    case "inicio":
      return "atividades";
    case "projeto":
      return "visao-geral";
    case "base":
      return "base";
    case "analise":
      return "analises";
    case "configuracoes":
      return "configuracoes";
    case "configurar":
      return "configurar-projeto";
    case "demanda":
    case "demandas":
      return "demandas";
    case "mapa":
    case "vincular":
      return "funcionalidades";
    case "codigo":
      return "codigo";
  }
}

/** O projeto que a tela pede, quando pede um. */
const projetoDaTela = (t: Tela): string | undefined => ("projeto" in t ? t.projeto : undefined);

/**
 * Aba que fica montada (escondida) quando outra coisa está à vista: a de uma análise
 * aberta e a do vínculo das issues, cujo terminal roda o agente. Desmontar
 * encerraria o processo (#212).
 */
const ficaViva = (a: Aba) =>
  (a.tela.tipo === "analise" && !!a.tela.aberta) || a.tela.tipo === "vincular";

const nomeDoArquivo = (caminho: string) =>
  (caminho.split("/").pop() ?? caminho).replace(/\.md$/, "");

/** De quanto em quanto tempo o menu e o seletor releem as contagens. */
const INTERVALO_CONTAGENS = 5000;

const ESTREITA = "(max-width: 900px)";
const telaEstreita = () => typeof matchMedia === "function" && matchMedia(ESTREITA).matches;

/**
 * Navegação por projeto (#319, §5.2 da revisão de UX), no shell do Control Tower:
 * barra de título com o seletor de projeto; menu lateral com as seções do projeto
 * (Visão geral, Atividades, Análises, Demandas), do conhecimento (Base,
 * Funcionalidades, Código) e Configurações no rodapé, com rótulos e contagens;
 * área principal e barra de status. As seções trocam no lugar e mostram o projeto
 * escolhido; só os objetos (análise, documento, demanda) abrem aba. O app abre no
 * último projeto usado. Cada tela pede às outras o que abrir por `navegar`.
 * Abaixo de 900 px o menu vira gaveta.
 *
 * Enquanto uma seção nova não existe, o item do menu abre a tela atual: Visão geral
 * mostra a tela Projeto (#320).
 * Atividades (#321) mostra os grupos do que espera o analista e, abaixo, o resto do
 * Início (nova análise, recentes, projetos).
 */
export function Navegacao({
  inicial,
  api = apiConhecimentoTauri,
  apiAnalise,
  apiConfiguracoes,
  apiMapa = apiMapaTauri,
  apiTerminal,
  criarTela,
  abrirLink,
  aoPrimeiroUso,
  refazerPrimeiroUso,
  produto = "Ctower Business Agents",
  versao = null,
  banner,
}: Props) {
  const contador = useRef(0);
  const [projetoPedido, setProjeto] = useState<string>(
    () => (inicial && projetoDaTela(inicial)) || ultimoProjeto(),
  );
  const [secao, setSecao] = useState<SecaoAberta>(() => ({
    id: inicial ? secaoDaTela(inicial) : "visao-geral",
    tela: inicial && !chaveDoObjeto(inicial) ? inicial : { tipo: "projeto" },
    vez: 0,
  }));
  const [abas, setAbas] = useState<Aba[]>(() => {
    const chave = inicial && chaveDoObjeto(inicial);
    return chave && inicial ? [{ chave, tela: inicial, vez: 0 }] : [];
  });
  /** Aba de objeto à vista; `null` mostra a seção. */
  const [ativa, setAtiva] = useState<string | null>(() => (inicial && chaveDoObjeto(inicial)) || null);
  /** Projetos com o mapa de funcionalidades montado: a sessão do Claude continua viva. */
  const [mapas, setMapas] = useState<string[]>([]);
  const [projetos, setProjetos] = useState<ProjetoResumo[]>([]);
  const [lidos, setLidos] = useState(false);
  const [inicio, setInicio] = useState<Inicio | null>(null);
  const [falha, setFalha] = useState<string | null>(null);
  // Muda a cada navegação e quando as configurações mexem nos projetos ou na base.
  const [recarga, setRecarga] = useState(0);
  const [ladoAberto, setLadoAberto] = useState(() => !telaEstreita());
  /** Resumo do projeto escolhido (#318): alimenta as Atividades e o selo do menu. */
  const [resumo, setResumo] = useState<{ projeto: string; resumo: ResumoProjeto } | null>(null);
  const [falhaResumo, setFalhaResumo] = useState<{ projeto: string; falha: string } | null>(null);
  /** Ambiente do Claude conferido por projeto: o grupo "Ambiente a preparar". */
  const [ambientes, setAmbientes] = useState<Record<string, AmbienteConferido>>({});

  const aba = ativa ? (abas.find((a) => a.chave === ativa) ?? null) : null;

  // Com muitas abas a faixa rola: a aba ativa sempre fica à vista.
  const faixaAbas = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const ativaEl = faixaAbas.current?.querySelector<HTMLElement>('.etab[data-on="true"]');
    ativaEl?.scrollIntoView?.({ block: "nearest", inline: "nearest" });
  }, [ativa, abas.length]);

  // Projeto escolhido: o pedido, o último usado ou, se ele sumiu, o primeiro da lista.
  // Antes de ler os projetos, nenhum: as seções esperam a lista.
  const projeto = !lidos
    ? ""
    : projetos.some((p) => p.id === projetoPedido)
      ? projetoPedido
      : (projetos[0]?.id ?? "");

  useEffect(() => {
    if (projeto) lembrarProjeto(projeto);
  }, [projeto]);

  // O mapa de funcionalidades fica montado depois de visto: escondê-lo não encerra a
  // sessão de mapeamento com o Claude (#257).
  const mapaAVista = secao.id === "funcionalidades" && ativa === null && !!projeto;
  useEffect(() => {
    if (mapaAVista) setMapas((m) => (m.includes(projeto) ? m : [...m, projeto]));
  }, [mapaAVista, projeto]);
  const mapasMontados = mapaAVista && !mapas.includes(projeto) ? [...mapas, projeto] : mapas;

  // Alterações sem salvar num editor (configurações, documento da base): sair da
  // tela pede confirmação. `saida` guarda a ação à espera da resposta.
  const pendenteRef = useRef(false);
  const [saida, setSaida] = useState<(() => void) | null>(null);
  const aoPendencia = useCallback((p: boolean) => {
    pendenteRef.current = p;
  }, []);
  const sairDaTela = useCallback((acao: () => void) => {
    if (pendenteRef.current) setSaida(() => acao);
    else acao();
  }, []);

  // Conversa de cada aba de análise aberta: fechar a aba com ela viva pede confirmação.
  const conversas = useRef(new Map<string, EstadoConversa>());
  const [fechamento, setFechamento] = useState<{ chave: string; trabalhando: boolean } | null>(null);
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
    const vez = ++contador.current;
    const chave = chaveDoObjeto(t);
    if (chave) {
      setAbas((atuais) => {
        const existente = atuais.find((a) => a.chave === chave);
        if (!existente) return [...atuais, { chave, tela: t, vez }];
        // A análise já aberta só ganha o foco: abrir do zero encerraria a conversa (#212).
        if (ficaViva(existente)) return atuais;
        return atuais.map((a) => (a.chave === chave ? { chave, tela: t, vez } : a));
      });
      setAtiva(chave);
    } else {
      // Seção: troca no lugar; pedir outro projeto troca o projeto da barra de título.
      const p = projetoDaTela(t);
      if (p) setProjeto(p);
      setSecao({ id: secaoDaTela(t), tela: t, vez });
      setAtiva(null);
    }
    setRecarga((r) => r + 1);
    // Na gaveta, escolher um item fecha o menu.
    if (telaEstreita()) setLadoAberto(false);
  }, []);

  const navegar = useCallback((t: Tela) => sairDaTela(() => navegarAgora(t)), [sairDaTela, navegarAgora]);

  useEffect(() => {
    let ativo = true;
    api
      .situacao()
      .then((s) => {
        if (!ativo) return;
        setProjetos(s.projetos);
        setLidos(true);
        setFalha(null);
      })
      .catch((e: unknown) => {
        if (!ativo) return;
        setFalha(mensagemDe(e));
        setLidos(true);
      });
    api
      .inicio()
      .then((i) => ativo && setInicio(i))
      // O início mostra a própria falha; o menu só fica sem as contagens.
      .catch(() => ativo && setInicio(null));
    return () => {
      ativo = false;
    };
  }, [api, recarga]);

  // As contagens acompanham o que acontece fora das telas (análise criada, agente que
  // passou a esperar o analista, base editada): relê o início quando a base muda e,
  // por garantia, a cada poucos segundos (#227).
  useEffect(() => {
    let ativo = true;
    const ler = () =>
      api
        .inicio()
        .then((i) => ativo && setInicio(i))
        .catch(() => undefined);
    const relogio = setInterval(ler, INTERVALO_CONTAGENS);
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

  // O resumo do projeto escolhido acompanha o diário de eventos (#321): cada aviso da
  // vigia das análises (sessão que passou a esperar o analista, análise concluída ou
  // aprovada) relê o resumo, e o selo de Atividades muda na hora. A base editada fora
  // do app e o relógio das contagens também releem.
  const vigiaAnalises = apiAnalise ?? apiAnaliseTauri;
  useEffect(() => {
    if (!projeto) return;
    let ativo = true;
    const ler = () =>
      api
        .resumoProjeto(projeto)
        .then((r) => {
          if (!ativo) return;
          setResumo({ projeto, resumo: r });
          setFalhaResumo(null);
        })
        .catch((e: unknown) => ativo && setFalhaResumo({ projeto, falha: mensagemDe(e) }));
    ler();
    const relogio = setInterval(ler, INTERVALO_CONTAGENS);
    const paradas: (() => void)[] = [];
    const guardar = (p: () => void) => (ativo ? paradas.push(p) : p());
    vigiaAnalises
      .vigiar(projeto, ler)
      .then(guardar)
      .catch(() => undefined);
    api
      .vigiarBase(ler)
      .then(guardar)
      .catch(() => undefined);
    return () => {
      ativo = false;
      clearInterval(relogio);
      paradas.forEach((p) => p());
    };
  }, [api, vigiaAnalises, projeto, recarga]);

  // O ambiente do Claude do projeto, conferido de novo a cada navegação: o preparo
  // feito em "Configurar projeto" some das Atividades ao voltar.
  useEffect(() => {
    if (!projeto) return;
    let ativo = true;
    api
      .verificarAmbiente(projeto)
      .then((a) => ativo && setAmbientes((m) => ({ ...m, [projeto]: a })))
      .catch(() => ativo && setAmbientes((m) => ({ ...m, [projeto]: null })));
    return () => {
      ativo = false;
    };
  }, [api, projeto, recarga]);

  /**
   * Item do menu: mostra a seção. A mesma seção, com uma aba na frente, só volta a
   * ela, sem abrir do zero.
   */
  const irParaSecao = (s: Secao) => {
    if (secao.id === s) {
      if (ativa !== null) {
        sairDaTela(() => {
          setAtiva(null);
          if (telaEstreita()) setLadoAberto(false);
        });
      }
      return;
    }
    navegar(TELA_DA_SECAO[s]);
  };

  /** Seletor de projeto: as seções passam a mostrar o projeto escolhido. */
  const trocarProjeto = (p: string) =>
    sairDaTela(() => {
      setProjeto(p);
      // A seção à vista abre do zero, no projeto novo.
      setSecao((s) => ({ id: s.id, tela: TELA_DA_SECAO[s.id], vez: ++contador.current }));
      setAtiva(null);
    });

  const ativar = (chave: string) => {
    if (chave === ativa) return;
    sairDaTela(() => {
      setAtiva(chave);
      if (telaEstreita()) setLadoAberto(false);
    });
  };

  const fechar = (chave: string) => {
    const conversa = conversas.current.get(chave);
    if (conversa) {
      setFechamento({ chave, trabalhando: conversa === "trabalhando" });
      return;
    }
    if (chave === ativa && pendenteRef.current) {
      sairDaTela(() => fecharAgora(chave));
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
    if (ativa && chaves.includes(ativa)) setAtiva(resto.some((a) => a.chave === manter) ? manter : null);
  };

  /** Fecha a aba; sendo a da frente, a vizinha passa à frente, ou a seção sem abas. */
  const fecharAgora = (chave: string) => {
    conversas.current.delete(chave);
    const i = abas.findIndex((a) => a.chave === chave);
    const resto = abas.filter((a) => a.chave !== chave);
    setAbas(resto);
    if (chave === ativa) setAtiva(resto[Math.min(i, resto.length - 1)]?.chave ?? null);
  };

  const nomeProjeto = (id: string) =>
    projetos.find((s) => s.id === id)?.nome ?? inicio?.projetos.find((s) => s.id === id)?.nome ?? id;

  const tituloDaAba = (t: Tela): string => {
    if (t.tipo === "base" && t.documento) return nomeDoArquivo(t.documento);
    if (t.tipo === "analise" && t.aberta) return t.aberta.id;
    if (t.tipo === "demanda") return t.demanda;
    if (t.tipo === "vincular") return `Vincular demandas · ${nomeProjeto(t.projeto)}`;
    return ROTULO_SECAO[secaoDaTela(t)];
  };

  const resumoAtual = resumo?.projeto === projeto ? resumo.resumo : null;
  const falhaAtual = falhaResumo?.projeto === projeto ? falhaResumo.falha : null;
  const ambienteAtual = ambientes[projeto];
  /** O selo de Atividades do projeto escolhido, pelo resumo; `null` antes de lê-lo. */
  const selo = resumoAtual ? seloAtividades(resumoAtual, ambienteAtual) : null;

  const noSeletor: ProjetoNoSeletor[] = projetos.map((p) => {
    const visto = inicio?.projetos.find((x) => x.id === p.id);
    return {
      id: p.id,
      nome: p.nome,
      pendencias:
        p.id === projeto && selo !== null
          ? selo
          : inicio && visto
            ? pendenciasNoInicio(inicio, visto)
            : 0,
      erro: !!visto?.erro,
    };
  });
  const contagensDoInicio = contagensDoMenu(inicio, projeto);
  const contagens =
    selo === null
      ? contagensDoInicio
      : { ...contagensDoInicio, atividades: { n: selo, atencao: selo > 0 } };
  const aRevalidar = inicio?.projetos.reduce((n, s) => n + s.aRevalidar, 0) ?? 0;
  const sessoesAguardando =
    inicio?.emAndamento.filter((a) => a.sessao?.estado === "aguardando-analista") ?? [];
  const aguardando = sessoesAguardando.length;
  // As seções esperam a lista de projetos, para não abrir num projeto que sumiu.
  const secaoAVista = lidos && ativa === null;
  const chaveSecao = `${secao.id}:${projeto}:${secao.vez}`;
  const tela = secao.tela;

  return (
    <div className="shell">
      <Titlebar
        produto={produto}
        versao={versao}
        antes={
          <button
            type="button"
            className="ibtn"
            aria-label={ladoAberto ? "Fechar menu lateral" : "Abrir menu lateral"}
            aria-expanded={ladoAberto}
            aria-controls="menu-lateral"
            onClick={() => setLadoAberto((v) => !v)}
          >
            <Icon name="sidebar" />
          </button>
        }
        meio={<SeletorProjeto projetos={noSeletor} atual={projeto} aoEscolher={trocarProjeto} />}
      />

      <div className="shell-work">
        <div id="menu-lateral" className="sidepanel menu-painel" hidden={!ladoAberto}>
          <MenuLateral
            atual={ativa === null ? ITEM_DA_SECAO(secao.id) : null}
            contagens={contagens}
            aoEscolher={irParaSecao}
            aoNovaAnalise={projeto ? () => navegar({ tipo: "analise", projeto, nova: true }) : undefined}
            rodape={
              refazerPrimeiroUso && (
                <button type="button" className="link menu-refazer" onClick={() => sairDaTela(refazerPrimeiroUso)}>
                  Refazer o primeiro uso
                </button>
              )
            }
          />
        </div>
        {ladoAberto && (
          /* Gaveta (tela estreita): tocar fora fecha. */
          <div className="sidepanel-scrim" aria-hidden="true" onClick={() => setLadoAberto(false)} />
        )}

        <div className="shell-main">
          {banner}
          {falha && (ativa !== null || secao.id !== "atividades") && (
            <div className="banner" role="alert">
              {falha}
            </div>
          )}
          {abas.length > 0 && (
            <div className="etabs-bar">
              <div
                className="etabs"
                role="tablist"
                aria-label="Abas abertas"
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
                        <Icon name={ICONE_OBJETO[a.tela.tipo] ?? "file"} size={14} />
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
                  onClick={() => sairDaTela(() => fecharOutras(aba.chave))}
                >
                  Fechar outras
                </button>
              )}
            </div>
          )}
          <main className="editor" id="area-editor" aria-label={aba ? tituloDaAba(aba.tela) : ROTULO_SECAO[secao.id]}>
            <div className="editor-tela">
              {/* Seção à vista: só uma montada por vez, no projeto escolhido. */}
              {ativa === null && !lidos && (
                <div className="page">
                  <p className="muted">Carregando…</p>
                </div>
              )}
              {secaoAVista && secao.id === "visao-geral" && (
                <VisaoGeral
                  key={chaveSecao}
                  projeto={projeto}
                  nomeProjeto={nomeProjeto(projeto)}
                  resumo={resumoAtual}
                  falha={falhaAtual}
                  ambiente={ambienteAtual}
                  api={api}
                  apiConfiguracoes={apiConfiguracoes ?? apiConfiguracoesTauri}
                  apiMapa={apiMapa}
                  navegar={navegar}
                />
              )}
              {secaoAVista && secao.id === "atividades" && (
                <div key={chaveSecao}>
                  <TelaAtividades
                    projeto={projeto}
                    nomeProjeto={nomeProjeto(projeto)}
                    resumo={resumoAtual}
                    falha={falhaAtual}
                    ambiente={ambienteAtual}
                    navegar={navegar}
                  />
                  <TelaInicio
                    api={api}
                    navegar={navegar}
                    abrirLink={abrirLink}
                    aoPrimeiroUso={aoPrimeiroUso}
                    apiProjeto={apiConfiguracoes}
                    aoMudarProjetos={() => setRecarga((r) => r + 1)}
                    semConducao
                  />
                </div>
              )}
              {secaoAVista && secao.id === "analises" && (
                <TelaAnalise
                  key={chaveSecao}
                  api={apiAnalise}
                  apiTerminal={apiTerminal}
                  criarTela={criarTela}
                  projetoInicial={projeto}
                  perguntaInicial={tela.tipo === "analise" ? tela.pergunta : undefined}
                  situacaoInicial={tela.tipo === "analise" ? tela.situacao : undefined}
                  novaInicial={tela.tipo === "analise" ? tela.nova : undefined}
                  apiConhecimento={api}
                  aoPrimeiroUso={aoPrimeiroUso}
                  aoAbrirAnalise={(p, id, abertura) => navegar({ tipo: "analise", projeto: p, aberta: { id, abertura } })}
                />
              )}
              {secaoAVista && secao.id === "demandas" && (
                <div className="page" key={chaveSecao}>
                  {projeto ? (
                    <TelaDemandas
                      api={api}
                      projeto={projeto}
                      navegar={navegar}
                      abrirLink={abrirLink}
                      situacaoInicial={tela.tipo === "demandas" ? tela.situacao : undefined}
                    />
                  ) : (
                    <>
                      <h2>Demandas</h2>
                      <p className="muted">Nenhum projeto cadastrado.</p>
                    </>
                  )}
                </div>
              )}
              {secaoAVista && secao.id === "base" && (
                <TelaBase
                  key={chaveSecao}
                  api={api}
                  projetos={projetos}
                  projetoInicial={projeto}
                  navegar={navegar}
                  aoPendencia={aoPendencia}
                />
              )}
              {secaoAVista && secao.id === "codigo" && (
                <TelaCodigo key={chaveSecao} api={api} projeto={projeto} />
              )}
              {secaoAVista && secao.id === "configuracoes" && (
                <TelaConfiguracoes
                  key={chaveSecao}
                  api={apiConfiguracoes}
                  aoMudarProjetos={() => setRecarga((r) => r + 1)}
                  aoPendencia={aoPendencia}
                  listarProjetos={false}
                  aoConfigurarProjeto={(p) => navegar({ tipo: "configurar", projeto: p })}
                />
              )}
              {secaoAVista && secao.id === "configurar-projeto" && (
                <TelaConfiguracoes
                  key={chaveSecao}
                  api={apiConfiguracoes}
                  aoMudarProjetos={() => setRecarga((r) => r + 1)}
                  aoPendencia={aoPendencia}
                  projetoFixo={projeto}
                  aoVoltar={() => navegar({ tipo: "projeto", projeto })}
                  aoConfigurarProjeto={(p) => navegar({ tipo: "configurar", projeto: p })}
                  aoIrParaConfiguracoes={() => navegar({ tipo: "configuracoes" })}
                />
              )}
              {/* O mapa de cada projeto visto fica montado, escondido fora da vez: a
                  sessão de mapeamento continua enquanto o analista está noutra seção. */}
              {mapasMontados.map((p) => (
                <div key={`mapa:${p}`} className="editor-aba" hidden={!(mapaAVista && p === projeto)}>
                  <TelaMapa
                    projetos={projetos.filter((x) => x.id === p)}
                    projetoInicial={p}
                    apiAnalise={apiAnalise}
                    apiTerminal={apiTerminal}
                    criarTela={criarTela}
                    navegar={navegar}
                  />
                </div>
              ))}

              {/* Objetos abertos em abas. */}
              {aba?.tela.tipo === "base" && (
                <TelaBase
                  key={aba.vez}
                  api={api}
                  projetos={projetos}
                  documentoInicial={aba.tela.documento}
                  navegar={navegar}
                  aoPendencia={aoPendencia}
                />
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
              {/* As análises abertas ficam montadas, escondidas fora da vez: o agente
                  continua trabalhando enquanto o analista está noutra tela (#212). */}
              {abas.filter(ficaViva).map((a) => {
                const vinculado = a.tela.tipo === "vincular" ? a.tela.projeto : "";
                return a.tela.tipo === "analise" ? (
                  <div key={a.chave} className="editor-aba" hidden={a.chave !== ativa}>
                    <TelaAnalise
                      key={a.vez}
                      api={apiAnalise}
                      apiTerminal={apiTerminal}
                      criarTela={criarTela}
                      projetoInicial={a.tela.projeto}
                      aberturaInicial={a.tela.aberta}
                      aoPrimeiroUso={aoPrimeiroUso}
                      aoAbrirAnalise={(p, id, abertura) =>
                        navegar({ tipo: "analise", projeto: p, aberta: { id, abertura } })
                      }
                      aoVoltar={(p) => navegar({ tipo: "analise", projeto: p })}
                      aoMudarConversa={(estado) => aoMudarConversa(a.chave, estado)}
                    />
                  </div>
                ) : a.tela.tipo === "vincular" ? (
                  <div key={a.chave} className="editor-aba" hidden={a.chave !== ativa}>
                    <div className="page">
                      <VincularIssues
                        key={a.vez}
                        projeto={a.tela.projeto}
                        api={apiMapaTauri}
                        // Volta ao mapa do projeto, que continua montado.
                        aoVoltar={() => navegar({ tipo: "mapa", projeto: vinculado })}
                        aoVincular={() => undefined}
                        apiTerminal={apiTerminal}
                        criarTela={criarTela}
                      />
                    </div>
                  </div>
                ) : null;
              })}
            </div>
          </main>
        </div>
      </div>

      {saida && (
        <DialogoConfirmacao
          titulo="Alterações não salvas"
          aoCancelar={() => setSaida(null)}
          acoes={
            <>
              <button type="button" className="btn btn-primary" autoFocus onClick={() => setSaida(null)}>
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
          <p>Há um editor com alterações que não foram salvas. Sair agora as descarta.</p>
        </DialogoConfirmacao>
      )}

      {fechamento && (
        <DialogoConfirmacao
          titulo="Fechar a análise"
          aoCancelar={() => setFechamento(null)}
          acoes={
            <>
              <button type="button" className="btn btn-primary" autoFocus onClick={() => setFechamento(null)}>
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
              : "A conversa com o agente continua aberta nesta análise. Encerrar a conversa?"}
          </p>
          <p className="muted">
            Fechar a aba encerra o Claude Code desta análise. Dá para retomar a conversa depois, pela
            própria análise.
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
            Fechar uma aba encerra o Claude Code dela; dá para retomar a conversa depois, pela própria
            análise. “Manter as conversas” fecha só as abas sem conversa.
          </p>
        </DialogoConfirmacao>
      )}

      <footer className="statusbar" aria-label="Barra de status">
        <span className="sb-item mono" title={inicio?.base ?? undefined}>
          {inicio?.base ? `Base: ${inicio.base}` : "Base não escolhida"}
        </span>
        <span className="sb-item">{projetos.length === 1 ? "1 projeto" : `${projetos.length} projetos`}</span>
        <span className="sb-spacer" />
        {aguardando > 0 && (
          <button
            type="button"
            className="sb-item sb-warn"
            title="Abre as Atividades, onde fica cada sessão que espera você"
            onClick={() => {
              // O projeto da barra de título, se uma das sessões é dele; senão, o da primeira.
              const alvo = sessoesAguardando.some((a) => a.projeto === projeto)
                ? projeto
                : sessoesAguardando[0].projeto;
              navegar({ tipo: "inicio", projeto: alvo });
            }}
          >
            <Icon name="warning" size={12} />
            {aguardando === 1 ? "1 sessão aguarda você" : `${aguardando} sessões aguardam você`}
          </button>
        )}
        {aRevalidar > 0 && (
          <button
            type="button"
            className="sb-item sb-warn"
            onClick={() => {
              const s = inicio?.projetos.find((x) => x.id === projeto && x.aRevalidar > 0) ??
                inicio?.projetos.find((x) => x.aRevalidar > 0);
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

/** O pedido que cada item do menu faz (no projeto da barra de título). */
const TELA_DA_SECAO: Record<Secao, Tela> = {
  "visao-geral": { tipo: "projeto" },
  atividades: { tipo: "inicio" },
  analises: { tipo: "analise" },
  demandas: { tipo: "demandas" },
  base: { tipo: "base" },
  funcionalidades: { tipo: "mapa" },
  codigo: { tipo: "codigo" },
  configuracoes: { tipo: "configuracoes" },
  "configurar-projeto": { tipo: "configurar" },
};
