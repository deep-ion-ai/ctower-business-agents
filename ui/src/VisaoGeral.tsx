import { useEffect, useState, type ReactNode } from "react";
import { AjudaTermo } from "./AjudaTermo";
import { mensagemDe } from "./analise";
import type { ApiConhecimento } from "./conhecimento";
import type { Cobertura, CoberturaArea } from "./cobertura";
import type { ApiConfiguracoes, ProjetoConfig } from "./configuracoes";
import { ROTULO_ESTADO_RASTREADOR, quandoConferido, type DemandaVista } from "./demandas";
import { contar } from "./formatacao";
import type { MapaFuncionalidades } from "./funcionalidades";
import type { Tela } from "./Navegacao";
import type { ResumoProjeto } from "./resumoProjeto";
import { ambienteAPreparar, type AmbienteConferido } from "./atividades";
import {
  itensDeSaude,
  resumoDoProjeto,
  type EstadoDoProjeto,
  type ItemSaude,
} from "./saudeProjeto";
import {
  atividadeRecente,
  barrasPorTipo,
  horaDoResumo,
  proximosPassos,
  quando,
  segmentosDeAnalises,
  segmentosDeDemandas,
  type DestinoPasso,
  type Passo,
  type Segmento,
  type Tom,
} from "./visaoGeralLogica";
import "./visaoGeral.css";

/** Quantas áreas a cobertura mostra e quantas demandas o bloco lista. */
const AREAS_VISIVEIS = 6;
const DEMANDAS_VISIVEIS = 3;

const SIMBOLO_SAUDE = { ok: "✓", problema: "!", neutro: "·" } as const;
const TOM_SAUDE: Record<ItemSaude["estado"], Tom> = {
  ok: "done",
  problema: "attention",
  neutro: "neutro",
};

type ApiMapaLeitura = { mapa(projeto: string): Promise<MapaFuncionalidades> };

/** Resultado de uma leitura que pode falhar sem derrubar a tela. */
type Leitura<T> = { tipo: "lendo" } | { tipo: "ok"; valor: T } | { tipo: "falha"; mensagem: string };

function useLeitura<T>(ler: () => Promise<T>, chave: unknown): Leitura<T> {
  const [estado, setEstado] = useState<Leitura<T>>({ tipo: "lendo" });
  useEffect(() => {
    let ativo = true;
    // Relendo, mantém o que já estava à vista, sem piscar.
    setEstado((e) => (e.tipo === "ok" ? e : { tipo: "lendo" }));
    ler()
      .then((valor) => ativo && setEstado({ tipo: "ok", valor }))
      .catch((e: unknown) => ativo && setEstado({ tipo: "falha", mensagem: mensagemDe(e) }));
    return () => {
      ativo = false;
    };
    // `ler` muda de identidade a cada render: só a chave decide quando relê.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [chave]);
  return estado;
}

/**
 * Visão geral do projeto (#320, §6 da revisão de UX): responde "como o projeto
 * está" numa tela. O resumo (`resumo_projeto_app`, #318) é lido pela navegação, uma
 * vez por abertura e de novo a cada aviso de mudança, e compartilhado com as
 * Atividades e o selo do menu; a saúde do cadastro, a cobertura, o mapa e as demandas
 * vêm das suas fontes e falham cada uma por conta própria. Cada número e cada bloco
 * leva à lista ou ao objeto correspondente.
 */
export function VisaoGeral({
  projeto,
  nomeProjeto,
  resumo,
  falha,
  ambiente,
  api,
  apiConfiguracoes,
  apiMapa,
  navegar,
}: {
  projeto: string;
  nomeProjeto?: string;
  /** Resumo do projeto (#318); `null` enquanto carrega ou quando a leitura falhou. */
  resumo: ResumoProjeto | null;
  falha: string | null;
  /** Ambiente do Claude conferido pela navegação (`undefined`: ainda não; `null`: falhou). */
  ambiente: AmbienteConferido;
  api: ApiConhecimento;
  apiConfiguracoes: Pick<ApiConfiguracoes, "configuracoes" | "prontidaoRepositorios">;
  apiMapa: ApiMapaLeitura;
  navegar: (t: Tela) => void;
}) {
  // Saúde do cadastro: as mesmas fontes e a mesma conta das configurações.
  const [cadastro, setCadastro] = useState<ProjetoConfig | null>(null);
  const [prontidao, setProntidao] = useState<EstadoDoProjeto["prontidao"]>(undefined);
  useEffect(() => {
    let ativo = true;
    apiConfiguracoes
      .configuracoes()
      .then((c) => {
        if (!ativo) return;
        const p = c.projetos.find((x) => x.id === projeto) ?? null;
        setCadastro(p);
        if (p && p.repositorios.codigo.length === 0 && !p.repositorios.tracker) {
          // Nada a conferir na rede.
          setProntidao({ integracoes: [] });
        } else {
          apiConfiguracoes
            .prontidaoRepositorios(projeto)
            .then((r) => ativo && setProntidao(r))
            .catch(() => ativo && setProntidao(null));
        }
      })
      .catch(() => ativo && setCadastro(null));
    return () => {
      ativo = false;
    };
  }, [apiConfiguracoes, projeto]);

  const itens = cadastro ? itensDeSaude(cadastro, { ambiente, prontidao }) : [];

  // Relê a cobertura e as demandas quando o que o resumo conta muda, não a cada leitura dele.
  const assinatura = resumo
    ? [
        resumo.analises.total,
        resumo.demandas.abertas,
        resumo.demandas.encerradas,
        resumo.base.regras,
        resumo.base.aRevalidar,
        resumo.base.termos,
      ].join(":")
    : "";
  const cobertura = useLeitura<Cobertura>(
    () => (api.cobertura ? api.cobertura(projeto) : Promise.reject(new Error("indisponível"))),
    `${projeto}:${assinatura}`,
  );
  const mapa = useLeitura<MapaFuncionalidades>(() => apiMapa.mapa(projeto), projeto);
  const demandas = useLeitura<DemandaVista[]>(
    () => api.listarDemandas(projeto),
    `${projeto}:${assinatura}`,
  );

  const ir = {
    configurar: () => navegar({ tipo: "configurar", projeto }),
    novaAnalise: () => navegar({ tipo: "analise", projeto, nova: true }),
    aguardandoRevisao: () => navegar({ tipo: "analise", projeto, situacao: "concluida" }),
    atividades: () => navegar({ tipo: "inicio" }),
    analises: () => navegar({ tipo: "analise", projeto }),
    demandas: () => navegar({ tipo: "demandas", projeto }),
    base: () => navegar({ tipo: "base" }),
    mapa: () => navegar({ tipo: "mapa", projeto }),
    analise: (id: string) =>
      navegar({ tipo: "analise", projeto, aberta: { id, abertura: { tipo: "reabrir" } } }),
    demanda: (id: string) => navegar({ tipo: "demanda", projeto, demanda: id }),
  };
  const irParaPasso = (d: DestinoPasso) => {
    switch (d.tipo) {
      case "analise":
        return ir.analise(d.id);
      case "demandas":
        return ir.demandas();
      case "base":
        return ir.base();
      case "configurar":
        return ir.configurar();
      case "nova-analise":
        return ir.novaAnalise();
    }
  };

  const nome = nomeProjeto ?? cadastro?.nome ?? resumo?.nome ?? "";

  return (
    <section className="visao-geral page" aria-label="Visão geral do projeto">
      <header className="vg-cab">
        <div>
          <h2>{nome || "Visão geral"}</h2>
          <p className="vg-sub">
            {[
              cadastro ? resumoDoProjeto(cadastro) : null,
              resumo ? `resumo das ${horaDoResumo(resumo.geradoMs)}` : null,
            ]
              .filter(Boolean)
              .join(" · ")}
          </p>
        </div>
        <div className="vg-acoes">
          {itens.length > 0 && (
            <ul className="vg-legenda" aria-label="Saúde do projeto">
              {itens.map((i) => (
                <li key={i.id}>
                  <button
                    type="button"
                    className="vg-chip"
                    data-tone={TOM_SAUDE[i.estado]}
                    title={i.texto}
                    onClick={ir.configurar}
                  >
                    <span aria-hidden="true">{SIMBOLO_SAUDE[i.estado]}</span> {i.titulo}
                  </button>
                </li>
              ))}
            </ul>
          )}
          <button type="button" className="btn" onClick={ir.configurar}>
            Configurar projeto
          </button>
          <button type="button" className="btn btn-primary" onClick={ir.novaAnalise}>
            Nova análise
          </button>
        </div>
      </header>

      {falha && !resumo && (
        <div className="vg-vazio" role="alert">
          <p>
            <b>Não foi possível ler o resumo do projeto.</b> {falha}
          </p>
          <p>O app tenta de novo sozinho em alguns segundos.</p>
        </div>
      )}
      {!resumo && !falha && <p className="muted">Carregando…</p>}

      {resumo && (
        <>
          <ProximosPassos resumo={resumo} ambiente={ambiente} aoIr={irParaPasso} aoVerTodas={ir.atividades} />
          <Numeros resumo={resumo} ir={ir} />
          <div className="vg-grade vg-g32">
            <Analises resumo={resumo} aoAbrir={ir.analises} aoCriar={ir.novaAnalise} />
            <SessoesEAtividade resumo={resumo} aoAbrirAnalise={ir.analise} />
          </div>
          <div className="vg-grade vg-g2">
            <CoberturaDaBase cobertura={cobertura} mapa={mapa} aoVerLacunas={ir.mapa} />
            <Demandas resumo={resumo} demandas={demandas} aoAbrir={ir.demandas} aoAbrirDemanda={ir.demanda} />
          </div>
        </>
      )}
    </section>
  );
}

function Painel({
  titulo,
  rotulo,
  link,
  aoLink,
  children,
}: {
  titulo: ReactNode;
  rotulo: string;
  link?: string;
  aoLink?: () => void;
  children: ReactNode;
}) {
  return (
    <section className="panel vg-painel" aria-label={rotulo}>
      <h3>
        {titulo}
        {link && aoLink && (
          <button type="button" className="link" onClick={aoLink}>
            {link}
          </button>
        )}
      </h3>
      {children}
    </section>
  );
}

function ProximosPassos({
  resumo,
  ambiente,
  aoIr,
  aoVerTodas,
}: {
  resumo: ResumoProjeto;
  ambiente: AmbienteConferido;
  aoIr: (d: DestinoPasso) => void;
  aoVerTodas: () => void;
}) {
  const { visiveis, total } = proximosPassos(resumo, ambienteAPreparar(ambiente));
  return (
    <section className="panel vg-painel vg-passos" aria-label="Próximos passos">
      <h3>
        Próximos passos
        {total > 0 && (
          <button type="button" className="link" onClick={aoVerTodas}>
            {`Ver ${total === 1 ? "a atividade" : `as ${total} atividades`}`}
          </button>
        )}
      </h3>
      {visiveis.length === 0 ? (
        <div className="vg-vazio">
          <p>
            <b>Nada espera por você.</b> Quando o agente perguntar, terminar uma análise ou chegar uma demanda
            nova, o próximo passo aparece aqui.
          </p>
          <button type="button" className="btn btn-sm" onClick={aoVerTodas}>
            Abrir as atividades
          </button>
        </div>
      ) : (
        visiveis.map((p) => <PassoLinha key={p.chave} passo={p} aoIr={aoIr} />)
      )}
    </section>
  );
}

function PassoLinha({ passo, aoIr }: { passo: Passo; aoIr: (d: DestinoPasso) => void }) {
  return (
    <div className="vg-passo">
      <span className="vg-ponto" data-tone={passo.tom} aria-hidden="true" />
      <div className="vg-passo-texto">
        <p className="vg-passo-titulo">{passo.titulo}</p>
        <p className="vg-passo-descricao">{passo.descricao}</p>
      </div>
      <button
        type="button"
        className={passo.tom === "attention" ? "btn btn-primary" : "btn"}
        onClick={() => aoIr(passo.destino)}
      >
        {passo.acao}
      </button>
    </div>
  );
}

interface Irs {
  atividades: () => void;
  analises: () => void;
  aguardandoRevisao: () => void;
  demandas: () => void;
  base: () => void;
}

function Numeros({ resumo, ir }: { resumo: ResumoProjeto; ir: Irs }) {
  const revisao = resumo.aguardandoRevisao.length;
  const e = resumo.analises.porEstado;
  const d = resumo.demandas;
  const b = resumo.base;
  const kpis: {
    chave: string;
    rotulo: string;
    n: number;
    vazio: string;
    texto: string;
    tom?: Tom;
    ir: () => void;
  }[] = [
    {
      chave: "revisao",
      rotulo: "Aguardando sua revisão",
      n: revisao,
      vazio: "Quando o agente conclui uma análise, ela espera a sua revisão aqui.",
      texto: "análises concluídas pelo agente",
      tom: "attention",
      ir: ir.aguardandoRevisao,
    },
    {
      chave: "analises",
      rotulo: "Análises",
      n: resumo.analises.total,
      vazio: "Pergunte algo ao sistema para fazer a primeira.",
      texto: `${e.aprovada} aprovadas · ${e.emAndamento} em andamento`,
      ir: ir.analises,
    },
    {
      chave: "demandas",
      rotulo: "Demandas abertas",
      n: d.abertas,
      vazio: d.encerradas > 0 ? `${contar(d.encerradas, "encerrada", "encerradas")}.` : "Elas vêm do rastreador do projeto.",
      texto: `${d.semAnalise} sem análise · ${contar(d.encerradas, "encerrada", "encerradas")}`,
      ir: ir.demandas,
    },
    {
      chave: "regras",
      rotulo: "Regras na base",
      n: b.regras,
      vazio: "As afirmações confirmadas viram regras quando a análise é aprovada.",
      texto: `${b.aRevalidar} a revalidar · ${contar(b.termos, "termo", "termos")}`,
      ir: ir.base,
    },
  ];
  return (
    <div className="vg-grade vg-g4">
      {kpis.map((k) => (
        <button
          key={k.chave}
          type="button"
          className="panel vg-kpi"
          data-tone={k.n > 0 ? k.tom : undefined}
          onClick={k.ir}
        >
          <span className="vg-kpi-rotulo">{k.rotulo}</span>
          <span className="vg-kpi-valor" data-vazio={k.n === 0}>
            {k.n === 0 ? "Nenhuma" : k.n}
          </span>
          <span className="vg-kpi-texto">{k.n === 0 ? k.vazio : k.texto}</span>
        </button>
      ))}
    </div>
  );
}

function Pilha({ segmentos, rotulo }: { segmentos: Segmento[]; rotulo: string }) {
  return (
    <>
      <ul className="vg-pilha" aria-hidden="true">
        {segmentos.map((s) => (
          <li key={s.chave} data-tone={s.tom} style={{ flex: s.n }} title={`${s.rotulo}: ${s.n}`} />
        ))}
      </ul>
      <ul className="vg-legenda" aria-label={rotulo}>
        {segmentos.map((s) => (
          <li key={s.chave}>
            <i className="vg-seg" data-tone={s.tom} aria-hidden="true" />
            {s.rotulo} {s.n}
          </li>
        ))}
      </ul>
    </>
  );
}

function Analises({
  resumo,
  aoAbrir,
  aoCriar,
}: {
  resumo: ResumoProjeto;
  aoAbrir: () => void;
  aoCriar: () => void;
}) {
  const tipos = barrasPorTipo(resumo);
  return (
    <Painel titulo="Análises por situação" rotulo="Análises" link="Abrir as análises" aoLink={aoAbrir}>
      {resumo.analises.total === 0 ? (
        <div className="vg-vazio">
          <p>
            <b>Nenhuma análise ainda.</b> Uma análise é uma conversa com o agente sobre o código: ele lê, responde
            com evidências e o que você aprova vira regra na base.
          </p>
          <button type="button" className="btn btn-sm" onClick={aoCriar}>
            Nova análise
          </button>
        </div>
      ) : (
        <>
          <Pilha segmentos={segmentosDeAnalises(resumo)} rotulo="Análises por situação" />
          <h4 className="vg-subtitulo">Por tipo de análise</h4>
          <ul className="vg-barras" aria-label="Análises por tipo">
            {tipos.map((t) => (
              <li key={t.fluxo}>
                <span>{t.rotulo}</span>
                <span className="vg-trilho" title={contar(t.n, "análise", "análises")}>
                  <span style={{ width: `${t.largura}%` }} />
                </span>
                <span className="vg-valor">{t.n}</span>
              </li>
            ))}
          </ul>
        </>
      )}
    </Painel>
  );
}

const ROTULO_SESSAO = {
  ativa: "trabalhando",
  "aguardando-analista": "aguarda você",
} as const;
const TOM_SESSAO = { ativa: "running", "aguardando-analista": "attention" } as const;

function SessoesEAtividade({
  resumo,
  aoAbrirAnalise,
}: {
  resumo: ResumoProjeto;
  aoAbrirAnalise: (id: string) => void;
}) {
  const vivas = resumo.sessoes.porAnalise.flatMap((s) =>
    s.estado === "ativa" || s.estado === "aguardando-analista" ? [{ ...s, estado: s.estado }] : [],
  );
  const recentes = atividadeRecente(resumo);
  return (
    <Painel titulo="Sessões do agente" rotulo="Sessões e atividade">
      {vivas.length === 0 ? (
        <p className="vg-nota">
          Nenhuma sessão ativa. O agente trabalha numa sessão por análise; ela aparece aqui enquanto roda.
        </p>
      ) : (
        <ul className="vg-linhas" aria-label="Sessões ativas">
          {vivas.map((s) => (
            <li key={s.id}>
              <span className="vg-ponto" data-tone={TOM_SESSAO[s.estado]} aria-hidden="true" />
              <span className="vg-id">{s.analise}</span>
              <button type="button" className="link vg-titulo" onClick={() => aoAbrirAnalise(s.analise)}>
                Abrir a análise
              </button>
              <span className="vg-fim">
                <span className="vg-chip vg-chip-estatico" data-tone={TOM_SESSAO[s.estado]}>
                  {ROTULO_SESSAO[s.estado]}
                </span>
              </span>
            </li>
          ))}
        </ul>
      )}
      <h4 className="vg-subtitulo">Atividade recente</h4>
      {recentes.length === 0 ? (
        <p className="vg-nota">Ainda não aconteceu nada neste projeto.</p>
      ) : (
        <ul className="vg-linhas" aria-label="Atividade recente">
          {recentes.map((r) => (
            <li key={r.chave}>
              <span className="vg-quando">{quando(r.ms)}</span>
              <span className="vg-titulo">{r.texto}</span>
            </li>
          ))}
        </ul>
      )}
    </Painel>
  );
}

/** As áreas na ordem da tela: as com regras primeiro, depois as lacunas, da maior para a menor. */
function areasDaTela(areas: CoberturaArea[]): CoberturaArea[] {
  const comRegras = areas.filter((a) => !a.lacuna).sort((x, y) => y.regras - x.regras);
  const lacunas = areas.filter((a) => a.lacuna).sort((x, y) => y.calor - x.calor);
  return [...comRegras, ...lacunas].slice(0, AREAS_VISIVEIS);
}

function textoDaArea(a: CoberturaArea): string {
  if (a.lacuna) return "lacuna";
  if (a.regras > 0) return contar(a.regras, "regra", "regras");
  if (a.termos > 0) return contar(a.termos, "termo", "termos");
  return "sem regras";
}

function CoberturaDaBase({
  cobertura,
  mapa,
  aoVerLacunas,
}: {
  cobertura: Leitura<Cobertura>;
  mapa: Leitura<MapaFuncionalidades>;
  aoVerLacunas: () => void;
}) {
  const areas = cobertura.tipo === "ok" ? areasDaTela(cobertura.valor.areas) : [];
  const maior = Math.max(1, ...areas.map((a) => a.regras));
  return (
    <Painel
      titulo={
        <>
          Cobertura da base por área
          <AjudaTermo termo="cobertura">
            Quanto do código mais usado de cada área já tem regra ou documento na base.
          </AjudaTermo>
        </>
      }
      rotulo="Cobertura da base"
      link="Ver as lacunas"
      aoLink={aoVerLacunas}
    >
      {cobertura.tipo === "lendo" && <p className="muted">Calculando…</p>}
      {cobertura.tipo === "falha" && (
        <p className="error-text" role="alert">
          Não foi possível calcular a cobertura: {cobertura.mensagem}
        </p>
      )}
      {cobertura.tipo === "ok" &&
        (areas.length === 0 ? (
          <div className="vg-vazio">
            <p>
              <b>Nenhuma área na base ainda.</b> As áreas aparecem quando as regras e os termos aprovados em
              análises são classificados por área do negócio.
            </p>
          </div>
        ) : (
          <ul className="vg-barras" aria-label="Cobertura por área">
            {areas.map((a) => (
              <li key={a.id}>
                <span>{a.nome}</span>
                <span
                  className="vg-trilho"
                  title={
                    a.lacuna
                      ? `Lacuna: ${contar(a.arquivos, "arquivo", "arquivos")} e nenhuma regra`
                      : contar(a.regras, "regra", "regras")
                  }
                >
                  <span style={{ width: a.lacuna ? 0 : `${Math.max(3, Math.round((a.regras / maior) * 100))}%` }} />
                </span>
                <span className="vg-valor">{textoDaArea(a)}</span>
              </li>
            ))}
          </ul>
        ))}
      <EstadoDoMapa mapa={mapa} aoAbrir={aoVerLacunas} />
    </Painel>
  );
}

function EstadoDoMapa({ mapa, aoAbrir }: { mapa: Leitura<MapaFuncionalidades>; aoAbrir: () => void }) {
  if (mapa.tipo === "lendo") return <p className="muted">Lendo o mapa de funcionalidades…</p>;
  if (mapa.tipo === "falha")
    return (
      <p className="vg-nota" role="status">
        Não foi possível ler o mapa de funcionalidades: {mapa.mensagem}
      </p>
    );
  const m = mapa.valor;
  if (m.total === 0) {
    const pontos = m.completude?.semTratamento.length ?? 0;
    return (
      <div className="vg-vazio" aria-label="Mapa de funcionalidades">
        <b>Mapa de funcionalidades vazio</b>
        <span>
          {pontos > 0
            ? `${contar(pontos, "ponto de entrada do código", "pontos de entrada do código")} ainda não viraram funcionalidades com nome de negócio.`
            : "O código ainda não foi mapeado em funcionalidades com nome de negócio."}
        </span>
        <button type="button" className="btn btn-sm" onClick={aoAbrir}>
          Mapear com o Claude
        </button>
      </div>
    );
  }
  return (
    <p className="vg-nota" aria-label="Mapa de funcionalidades">
      Mapa de funcionalidades: {contar(m.total, "funcionalidade", "funcionalidades")} ·{" "}
      {m.documentadas} documentadas · {m.pendentes} a decidir.{" "}
      <button type="button" className="link" onClick={aoAbrir}>
        Abrir o mapa
      </button>
    </p>
  );
}

function Demandas({
  resumo,
  demandas,
  aoAbrir,
  aoAbrirDemanda,
}: {
  resumo: ResumoProjeto;
  demandas: Leitura<DemandaVista[]>;
  aoAbrir: () => void;
  aoAbrirDemanda: (id: string) => void;
}) {
  const d = resumo.demandas;
  const vazio = d.abertas + d.encerradas === 0;
  const abertas = demandas.tipo === "ok" ? demandas.valor.filter((x) => x.status === "aberta") : [];
  const conferida = abertas
    .map((x) => x.rastreador?.conferido)
    .filter((c): c is string => !!c)
    .sort()
    .reverse()[0];
  return (
    <Painel titulo="Demandas" rotulo="Demandas" link="Abrir as demandas" aoLink={aoAbrir}>
      {vazio ? (
        <div className="vg-vazio">
          <p>
            <b>Nenhuma demanda ainda.</b> Demandas vêm das issues do rastreador do projeto ou você cria uma na
            seção Demandas; cada uma pode ter análises ligadas.
          </p>
          <button type="button" className="btn btn-sm" onClick={aoAbrir}>
            Abrir as demandas
          </button>
        </div>
      ) : (
        <>
          <Pilha segmentos={segmentosDeDemandas(resumo)} rotulo="Demandas por situação" />
          {d.semAnalise > 0 && (
            <p className="vg-nota">{contar(d.semAnalise, "demanda aberta sem análise", "demandas abertas sem análise")}.</p>
          )}
          {demandas.tipo === "falha" && (
            <p className="vg-nota" role="status">
              Não foi possível listar as demandas: {demandas.mensagem}
            </p>
          )}
          {abertas.length > 0 && (
            <ul className="vg-linhas" aria-label="Demandas abertas">
              {abertas.slice(0, DEMANDAS_VISIVEIS).map((x) => (
                <li key={x.id}>
                  <span className="vg-id">{x.chave ?? x.id}</span>
                  <button type="button" className="link vg-titulo" onClick={() => aoAbrirDemanda(x.id)}>
                    {x.titulo}
                  </button>
                  {x.rastreador && (
                    <span className="vg-fim">
                      <span className="vg-chip vg-chip-estatico" data-tone={x.rastreador.estado === "em-andamento" ? "running" : undefined}>
                        {ROTULO_ESTADO_RASTREADOR[x.rastreador.estado]}
                      </span>
                    </span>
                  )}
                </li>
              ))}
            </ul>
          )}
          {conferida && (
            <p className="vg-nota">
              Estado conferido no rastreador em {quandoConferido(conferida)}. Nada é escrito no rastreador.
            </p>
          )}
        </>
      )}
    </Painel>
  );
}
