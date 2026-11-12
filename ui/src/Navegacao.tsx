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
import { ROTULO_SESSAO } from "./DocumentoAoVivo";
import type { Abertura } from "./PainelAnalise";
import { Icon, type IconName } from "./shell/icons";
import { Titlebar } from "./shell/Titlebar";
import { TelaAnalise } from "./TelaAnalise";
import type { ApiConfiguracoes } from "./configuracoes";
import { TelaBase } from "./TelaBase";
import { TelaConfiguracoes } from "./TelaConfiguracoes";
import { TelaInicio } from "./TelaInicio";
import { TelaSistema } from "./TelaSistema";
import type { SistemaResumo } from "./terminal";

/** Tela visível e o que ela abre. */
export type Tela =
  | { tipo: "inicio" }
  | { tipo: "sistema"; sistema?: string }
  | { tipo: "base"; documento?: string }
  | {
      tipo: "analise";
      sistema?: string;
      pergunta?: string;
      aberta?: { id: string; abertura: Abertura };
    }
  | { tipo: "configuracoes" };

interface Props {
  /** Tela ao abrir (ex.: a análise sugerida no fim do primeiro uso). */
  inicial?: Tela;
  api?: ApiConhecimento;
  /** Para testes. */
  apiAnalise?: ApiAnalise;
  /** Para testes. */
  apiConfiguracoes?: ApiConfiguracoes;
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
  { tipo: "sistema", rotulo: "Sistema", icone: "project" },
  { tipo: "base", rotulo: "Base de conhecimento", icone: "book" },
  { tipo: "analise", rotulo: "Análises", icone: "analysis" },
  { tipo: "configuracoes", rotulo: "Configurações", icone: "settings" },
];

const ROTULO_AREA = Object.fromEntries(AREAS.map((a) => [a.tipo, a.rotulo])) as Record<
  Tela["tipo"],
  string
>;
const ICONE_AREA = Object.fromEntries(AREAS.map((a) => [a.tipo, a.icone])) as Record<
  Tela["tipo"],
  IconName
>;

/** Aba aberta na área de editor: a tela que ela mostra e o contador de remontagem. */
interface Aba {
  chave: string;
  tela: Tela;
  /** Muda a cada navegação para a aba, para a tela abrir do zero com o pedido. */
  vez: number;
}

/** Uma aba por tela e por objeto aberto (sistema, documento ou análise). */
export function chaveDaTela(t: Tela): string {
  switch (t.tipo) {
    case "sistema":
      return t.sistema ? `sistema:${t.sistema}` : "sistema";
    case "base":
      return t.documento ? `doc:${t.documento}` : "base";
    case "analise":
      return t.aberta ? `analise:${t.sistema ?? ""}:${t.aberta.id}` : "analise";
    default:
      return t.tipo;
  }
}

const nomeDoArquivo = (caminho: string) =>
  (caminho.split("/").pop() ?? caminho).replace(/\.md$/, "");

/** Largura do painel lateral (mesmos limites do Control Tower). */
const SIDE_MIN = 170;
const SIDE_MAX = 600;
const SIDE_KEY = "cba.sideWidth";
const clampWidth = (w: number) => Math.min(SIDE_MAX, Math.max(SIDE_MIN, Math.round(w)));

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

/** Tom do ponto da análise: âmbar só quando a sessão espera o analista. */
function tomDaAnalise(a: AnaliseInicio): string | undefined {
  if (a.sessao?.estado === "aguardando-analista") return "attention";
  if (a.sessao?.estado === "ativa") return "running";
  if (a.estado === "aprovada") return "done";
  return undefined;
}

/**
 * Navegação depois do primeiro uso (spec, seção 10), no shell do Control Tower
 * (#85): barra de título, barra de atividades com as áreas (início, sistema, base,
 * análises e configurações), painel lateral com os sistemas, as análises e os
 * documentos abertos, área de editor com abas e barra de status. Cada tela pede às
 * outras o que abrir (um sistema, um documento, uma análise) por `navegar`, que
 * abre ou foca a aba do objeto. Abaixo de 900 px o painel lateral vira gaveta.
 */
export function Navegacao({
  inicial = { tipo: "inicio" },
  api = apiConhecimentoTauri,
  apiAnalise,
  apiConfiguracoes,
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
  const [sistemas, setSistemas] = useState<SistemaResumo[]>([]);
  const [inicio, setInicio] = useState<Inicio | null>(null);
  const [documentos, setDocumentos] = useState<string[]>(
    inicial.tipo === "base" && inicial.documento ? [inicial.documento] : [],
  );
  const [falha, setFalha] = useState<string | null>(null);
  // Muda a cada navegação e quando as configurações mexem nos sistemas ou na base.
  const [recarga, setRecarga] = useState(0);
  const [ladoAberto, setLadoAberto] = useState(() => !telaEstreita());
  const [largura, setLargura] = useState(larguraGravada);
  const contador = useRef(0);

  const aba = abas.find((a) => a.chave === ativa) ?? null;
  const tela = aba?.tela ?? null;

  /** Documentos abertos nesta sessão, o mais recente primeiro (painel lateral). */
  const lembrarDocumento = useCallback((doc: string) => {
    setDocumentos((ds) => (ds[0] === doc ? ds : [doc, ...ds.filter((d) => d !== doc)].slice(0, 20)));
  }, []);

  const navegar = useCallback((t: Tela) => {
    const chave = chaveDaTela(t);
    const vez = ++contador.current;
    setAbas((atuais) =>
      atuais.some((a) => a.chave === chave)
        ? atuais.map((a) => (a.chave === chave ? { chave, tela: t, vez } : a))
        : [...atuais, { chave, tela: t, vez }],
    );
    setAtiva(chave);
    if (t.tipo === "base" && t.documento) lembrarDocumento(t.documento);
    setRecarga((r) => r + 1);
    // Na gaveta, escolher um item fecha o painel.
    if (telaEstreita()) setLadoAberto(false);
  }, [lembrarDocumento]);

  useEffect(() => {
    let ativo = true;
    api
      .situacao()
      .then((s) => {
        if (!ativo) return;
        setSistemas(s.sistemas);
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
    if (existente) {
      setAtiva(existente.chave);
      if (telaEstreita()) setLadoAberto(false);
    } else navegar({ tipo } as Tela);
  };

  const fechar = (chave: string) => {
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

  const nomeSistema = (id: string) =>
    sistemas.find((s) => s.id === id)?.nome ??
    inicio?.sistemas.find((s) => s.id === id)?.nome ??
    id;

  const tituloDaAba = (t: Tela): string => {
    if (t.tipo === "sistema" && t.sistema) return nomeSistema(t.sistema);
    if (t.tipo === "base" && t.documento) return nomeDoArquivo(t.documento);
    if (t.tipo === "analise" && t.aberta) return t.aberta.id;
    return ROTULO_AREA[t.tipo];
  };

  const analises = inicio
    ? [
        ...inicio.emAndamento,
        ...inicio.recentes.filter(
          (r) => !inicio.emAndamento.some((e) => e.id === r.id && e.sistema === r.sistema),
        ),
      ]
    : [];
  const aRevalidar = inicio?.sistemas.reduce((n, s) => n + s.aRevalidar, 0) ?? 0;
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
              <GrupoLateral titulo="Sistemas" quantos={inicio?.sistemas.length ?? sistemas.length}>
                {(inicio?.sistemas ?? sistemas.map((s) => ({ ...s, aRevalidar: 0, erro: null }))).map(
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
                      selecionada={ativa === `sistema:${s.id}`}
                      onClick={() => navegar({ tipo: "sistema", sistema: s.id })}
                    />
                  ),
                )}
              </GrupoLateral>
              <GrupoLateral titulo="Análises" quantos={analises.length}>
                {analises.map((a) => (
                  <LinhaLateral
                    key={`${a.sistema}/${a.id}`}
                    ponto
                    rotulo={`${a.id}: ${a.titulo}`}
                    sub={
                      a.sessao && a.sessao.estado !== "encerrada"
                        ? `${a.sistemaNome} · ${ROTULO_SESSAO[a.sessao.estado]}`
                        : a.sistemaNome
                    }
                    tom={tomDaAnalise(a)}
                    selecionada={ativa === `analise:${a.sistema}:${a.id}`}
                    onClick={() =>
                      navegar({
                        tipo: "analise",
                        sistema: a.sistema,
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
                <button type="button" className="link" onClick={refazerPrimeiroUso}>
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
            <div className="etabs" role="tablist" aria-label="Abas abertas">
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
                      onClick={() => setAtiva(a.chave)}
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
                />
              )}
              {aba?.tela.tipo === "sistema" && (
                <TelaSistema
                  key={aba.vez}
                  api={api}
                  sistemas={sistemas}
                  sistemaInicial={aba.tela.sistema}
                  navegar={navegar}
                />
              )}
              {aba?.tela.tipo === "base" && (
                <TelaBase
                  key={aba.vez}
                  api={api}
                  sistemas={sistemas}
                  documentoInicial={aba.tela.documento}
                  navegar={navegar}
                  aoAbrirDocumento={lembrarDocumento}
                />
              )}
              {aba?.tela.tipo === "analise" && (
                <TelaAnalise
                  key={aba.vez}
                  api={apiAnalise}
                  sistemaInicial={aba.tela.sistema}
                  perguntaInicial={aba.tela.pergunta}
                  aberturaInicial={aba.tela.aberta}
                  aoPrimeiroUso={aoPrimeiroUso}
                />
              )}
              {aba?.tela.tipo === "configuracoes" && (
                <TelaConfiguracoes
                  key={aba.vez}
                  api={apiConfiguracoes}
                  aoMudarSistemas={() => setRecarga((r) => r + 1)}
                />
              )}
            </div>
          </div>
        </div>
      </div>

      <footer className="statusbar" aria-label="Barra de status">
        <span className="sb-item mono" title={inicio?.base ?? undefined}>
          {inicio?.base ? `Base: ${inicio.base}` : "Base não escolhida"}
        </span>
        <span className="sb-item">
          {sistemas.length === 1 ? "1 sistema" : `${sistemas.length} sistemas`}
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
              const s = inicio?.sistemas.find((x) => x.aRevalidar > 0);
              if (s) navegar({ tipo: "sistema", sistema: s.id });
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
