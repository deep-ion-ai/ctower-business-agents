import { useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import type {
  Afirmacao,
  Analise,
  EstadoAnalise,
  EstadoSessao,
  Evidencia,
  Diagrama,
  Pergunta,
  SecaoDocumento,
  SessaoVista,
  VistaAnalise,
} from "./analise";
import {
  AcoesAceite,
  AcoesPergunta,
  EnviarRespostas,
  AcoesTermo,
  type ComandoAfirmacao,
  type Revisao,
} from "./AcoesRevisao";
import {
  CartaoAfirmacao,
  ROTULO_CONFIANCA,
  SeletorEvidencias,
} from "./CartaoAfirmacao";
import { CenariosAoVivo } from "./CenariosAoVivo";
import { DiagramaAoVivo } from "./DiagramaAoVivo";
import { HistoriasAoVivo } from "./HistoriasAoVivo";
import { ManualAoVivo } from "./ManualAoVivo";
import { PrototipoAoVivo } from "./PrototipoAoVivo";
import { FaixaRevisao } from "./FaixaRevisao";
import {
  contagemPorFiltro,
  passaNoFiltro,
  pendente,
  resumir,
  type FiltroRevisao,
} from "./resumoRevisao";
import type { Pendencia } from "./revisao";
import { ordenarSecoes, tituloDaSecao } from "./secoesAnalise";
import { SumarioAnalise, type ItemSumario } from "./SumarioAnalise";
import { useAtalhosRevisao } from "./useAtalhosRevisao";
import { BotaoCodigo } from "./BotaoCodigo";
import { useEvidenciasVisiveis } from "./preferenciaEvidencias";

export { ROTULO_CONFIANCA };

export const ROTULO_SESSAO: Record<EstadoSessao, string> = {
  ativa: "Sessão ativa: o agente está trabalhando",
  "aguardando-analista": "Aguardando você: o Claude pede uma ação",
  concluida: "Resposta concluída: o agente espera a próxima mensagem",
  encerrada: "Sessão encerrada",
};

export const ROTULO_ANALISE: Record<EstadoAnalise, string> = {
  criada: "criada",
  "em-andamento": "em andamento",
  concluida: "concluída, aguardando revisão",
  aprovada: "aprovada",
};

export function SeloSessao({ sessao }: { sessao: SessaoVista | null }) {
  if (!sessao) {
    return (
      <p
        className="selo-sessao sem-sessao"
        role="status"
        aria-label="Estado da sessão"
        title="Nenhuma sessão ligada ainda: ela aparece quando o agente chama a primeira ferramenta."
      >
        Nenhuma sessão ligada ainda: ela aparece quando o agente chama a
        primeira ferramenta.
      </p>
    );
  }
  return (
    <p
      className="selo-sessao"
      role="status"
      aria-label="Estado da sessão"
      data-estado={sessao.estado}
      title={ROTULO_SESSAO[sessao.estado]}
    >
      {ROTULO_SESSAO[sessao.estado]}
    </p>
  );
}

export function chaveEvidencia(e: Evidencia): string {
  return `${e.arquivo}:${e.linhas}`;
}

/** Onde uma evidência foi escolhida: o trecho abre logo abaixo dela (#130). */
export type OrigemEvidencia =
  `afirmacao-${number}` | `sugestao-${number}` | `diagrama-${number}`;

/**
 * As evidências de uma origem, sem repetir: é por elas que a aba Código navega
 * (anterior e próxima) a partir da evidência aberta (#185).
 */
export function evidenciasDaOrigem(
  analise: Analise,
  origem: OrigemEvidencia,
): Evidencia[] {
  const [tipo, n] = origem.split("-");
  const numero = Number(n);
  let lista: Evidencia[];
  if (tipo === "afirmacao") {
    lista =
      analise.afirmacoes.find((a) => a.numero === numero)?.evidencias ?? [];
  } else if (tipo === "diagrama") {
    const d = (analise.diagramas ?? []).find((x) => x.numero === numero);
    lista = [...(d?.nos ?? []), ...(d?.arestas ?? [])].flatMap(
      (el) => el.evidencias ?? [],
    );
  } else {
    const e = analise.sugestoes.find((s) => s.numero === numero)?.evidencia;
    lista = e ? [e] : [];
  }
  return [...new Map(lista.map((e) => [chaveEvidencia(e), e])).values()];
}

interface Props {
  vista: VistaAnalise;
  /** Evidência aberta (destacada na lista). */
  evidenciaAberta?: string | null;
  aoAbrirEvidencia: (e: Evidencia, origem: OrigemEvidencia) => void;
  /** Fecha a evidência aberta (ao ocultar o código de onde ela veio). */
  aoFecharEvidencia?: () => void;
  /** Trecho da evidência aberta, mostrado abaixo do item em `origemTrecho`. */
  trecho?: ReactNode;
  origemTrecho?: OrigemEvidencia | null;
  /** Ações de revisão do analista (#37); sem elas, ou com a análise aprovada, só leitura. */
  revisao?: Revisao;
  /** Sem o cabeçalho (id, título, estado e sessão), quando a barra da tela já os mostra. */
  semCabecalho?: boolean;
  /**
   * Pendências da aprovação (a prontidão): a faixa mostra quantas são, pela mesma
   * conta do cabeçalho e do diálogo de aprovação (#222).
   */
  pendencias?: Pendencia[] | null;
}

const ID_PERGUNTAS = "secao-perguntas";
const ID_PROPOSTAS = "secao-propostas";

/** Documento da análise montado ao vivo a partir do estado gravado. */
export function DocumentoAoVivo({
  vista,
  evidenciaAberta,
  aoAbrirEvidencia,
  aoFecharEvidencia,
  trecho,
  origemTrecho,
  revisao,
  semCabecalho = false,
  pendencias,
}: Props) {
  const { analise } = vista;
  const deCenarios = analise.fluxo === "cenarios";
  const dePrototipo = analise.fluxo === "prototipo";
  // Histórias, cenários e o protótipo têm registros próprios, no lugar das afirmações.
  const deHistorias = analise.fluxo === "historias" || deCenarios || dePrototipo;
  // O manual do usuário (#272) tem afirmações e, além delas, o passo a passo.
  const deManual = analise.fluxo === "manual";
  const rotuloEntrada = deHistorias ? "Pedido" : deManual ? "Funcionalidade" : "Pergunta";
  // Aprovada, a revisão fecha: as regras já foram para a base.
  const rev = analise.estado === "aprovada" ? undefined : revisao;
  const diagramas = analise.diagramas ?? [];
  const vistos = new Map((vista.diagramas ?? []).map((d) => [d.numero, d]));
  const secoes = agrupar(
    analise.afirmacoes,
    diagramas.map((d) => d.secao),
    vista.secoes,
  );
  const titulo = (secao: string) => tituloDaSecao(secao, vista.secoes);
  const perguntas = [...analise.perguntas].sort(
    (a, b) =>
      Number(a.estado !== "aberta") - Number(b.estado !== "aberta") ||
      a.numero - b.numero,
  );
  const sugestoes = analise.sugestoes.filter((s) => s.estado !== "recusada");
  // As evidências das sugestões ficam recolhidas: o analista abre o código quando quer (#130).
  const [comCodigo, setComCodigo] = useState<ReadonlySet<OrigemEvidencia>>(
    new Set(),
  );
  // As das afirmações também (#404), salvo quando o seletor global as mostra: aí o
  // conjunto guarda as afirmações em que o analista as escondeu.
  const [evidenciasVisiveis, alternarEvidenciasVisiveis] =
    useEvidenciasVisiveis();
  const invertido = (o: OrigemEvidencia) =>
    evidenciasVisiveis && o.startsWith("afirmacao-");
  const codigoVisivel = (o: OrigemEvidencia) =>
    comCodigo.has(o) !== invertido(o) || origemTrecho === o;
  const alternarTodasEvidencias = () => {
    alternarEvidenciasVisiveis();
    setComCodigo(
      (atual) =>
        new Set([...atual].filter((o) => !o.startsWith("afirmacao-"))),
    );
  };
  const alternarCodigo = (o: OrigemEvidencia) => {
    setComCodigo((atual) => {
      const novo = new Set(atual);
      const marcar = codigoVisivel(o) ? invertido(o) : !invertido(o);
      if (marcar) novo.add(o);
      else novo.delete(o);
      return novo;
    });
    // Fechar o código fecha também o trecho aberto dali.
    if (codigoVisivel(o) && origemTrecho === o && evidenciaAberta) {
      const ev = evidenciasDaOrigem(analise, o).find(
        (e) => chaveEvidencia(e) === evidenciaAberta,
      );
      if (aoFecharEvidencia) aoFecharEvidencia();
      else if (ev) aoAbrirEvidencia(ev, o);
    }
  };

  // Revisão guiada (#184): progresso, filtros, sumário e atalhos de teclado.
  const [filtro, setFiltro] = useState<FiltroRevisao>("todos");
  const [atual, setAtual] = useState<number | null>(null);
  const [comando, setComando] = useState<ComandoAfirmacao | null>(null);
  const resumo = useMemo(
    () => resumir(analise.afirmacoes),
    [analise.afirmacoes],
  );
  const contagem = useMemo(
    () => contagemPorFiltro(analise.afirmacoes),
    [analise.afirmacoes],
  );
  const comFaixa = !!rev && !deHistorias && resumo.total > 0;
  // As afirmações que o filtro mostra, na ordem do documento: por elas J e K andam.
  const visiveis = secoes.flatMap(([, afs]) =>
    afs.filter((a) => passaNoFiltro(a, filtro)),
  );
  const posicaoAtual = visiveis.findIndex((a) => a.numero === atual);
  const afirmacaoAtual = posicaoAtual >= 0 ? visiveis[posicaoAtual] : undefined;

  // O foco acompanha a afirmação atual, sem tirá-lo de um botão do próprio cartão.
  const corpo = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (atual === null) return;
    const el = corpo.current?.querySelector<HTMLElement>(
      `[data-testid="afirmacao-${atual}"]`,
    );
    if (!el || el.contains(document.activeElement)) return;
    el.scrollIntoView?.({ block: "nearest" });
    el.focus({ preventScroll: true });
  }, [atual]);

  const ir = (numero: number | undefined) =>
    numero !== undefined && setAtual(numero);
  useAtalhosRevisao(comFaixa, {
    aoProxima: () =>
      ir(
        visiveis[
          posicaoAtual < 0 ? 0 : Math.min(posicaoAtual + 1, visiveis.length - 1)
        ]?.numero,
      ),
    aoAnterior: () => ir(visiveis[Math.max(posicaoAtual - 1, 0)]?.numero),
    aoConfirmar: () => {
      if (
        !rev ||
        rev.ocupado ||
        !afirmacaoAtual ||
        afirmacaoAtual.estado === "confirmada"
      )
        return;
      const proxima = visiveis[posicaoAtual + 1] ?? visiveis[posicaoAtual - 1];
      rev
        .aoDecidir({
          tipo: "afirmacao",
          numero: afirmacaoAtual.numero,
          estado: "confirmada",
        })
        // Decidida a afirmação, segue para a próxima do filtro.
        .then(() => ir(proxima?.numero))
        .catch(() => undefined);
    },
    aoIncorreta: () =>
      afirmacaoAtual &&
      setComando({ numero: afirmacaoAtual.numero, tipo: "incorreta" }),
    aoAprofundar: () =>
      afirmacaoAtual &&
      setComando({ numero: afirmacaoAtual.numero, tipo: "aprofundar" }),
  });

  const abertas = perguntas.filter((p) => p.estado === "aberta").length;
  const propostasPendentes =
    analise.termos.filter((t) => t.estado === "proposto").length +
    sugestoes.filter((s) => s.estado === "proposta").length;
  const comPropostas = !deHistorias || analise.termos.length > 0;

  const itensSumario: ItemSumario[] = deHistorias
    ? []
    : [
        ...secoes.map(([secao, afs]) => ({
          id: `secao-${secao}`,
          titulo: titulo(secao),
          pendencias: afs.filter(pendente).length,
          vazia: afs.length === 0,
        })),
        {
          id: ID_PERGUNTAS,
          titulo: "Perguntas",
          pendencias: abertas,
          vazia: perguntas.length === 0,
        },
        ...(comPropostas
          ? [
              {
                id: ID_PROPOSTAS,
                titulo: "Propostas para a base",
                pendencias: propostasPendentes,
                vazia: analise.termos.length + sugestoes.length === 0,
              },
            ]
          : []),
      ];
  // Sem afirmações (a revisão de um texto, por exemplo) o sumário só teria as perguntas.
  const comSumario = !deHistorias && secoes.length > 0;

  return (
    <article className="documento-ao-vivo" aria-label="Documento da análise">
      {!semCabecalho && (
        <header>
          <h2>
            {analise.id}: {analise.titulo}
          </h2>
          <p className="muted">
            Análise {ROTULO_ANALISE[analise.estado]} · versão {analise.versao}
          </p>
          <SeloSessao sessao={vista.sessao} />
        </header>
      )}

      {comFaixa && (
        <FaixaRevisao
          resumo={resumo}
          contagem={contagem}
          filtro={filtro}
          aoFiltrar={setFiltro}
          pendencias={pendencias ?? undefined}
        />
      )}

      <div
        className="documento-corpo"
        data-com-sumario={comSumario || undefined}
        ref={corpo}
      >
        {comSumario && <SumarioAnalise itens={itensSumario} />}
        <div className="documento-conteudo">
          {/* Na revisão, o texto revisado já aparece no parecer, com as marcas. */}
          {analise.fluxo !== "revisar" && (
            <section aria-label={rotuloEntrada}>
              <h3>{rotuloEntrada}</h3>
              <blockquote>{analise.entrada}</blockquote>
            </section>
          )}

          {deManual && (
            <ManualAoVivo
              projeto={analise.projeto}
              analise={analise.id}
              passos={analise.passos ?? []}
              revisao={rev}
            />
          )}

          {dePrototipo ? (
            <PrototipoAoVivo
              projeto={analise.projeto}
              analise={analise.id}
              versao={analise.versao}
              telas={analise.prototipo?.telas ?? []}
              origem={analise.origem}
              revisao={rev}
            />
          ) : deCenarios ? (
            <CenariosAoVivo
              tabelas={analise.tabelas ?? []}
              cenarios={analise.cenarios ?? []}
              origem={analise.origem}
              revisao={rev}
            />
          ) : deHistorias ? (
            <HistoriasAoVivo
              historias={analise.historias ?? []}
              origem={analise.origem}
              revisao={rev}
            />
          ) : (
            <section aria-label="Afirmações">
              <h3>Afirmações</h3>
              {analise.afirmacoes.some((a) => a.evidencias.length > 0) && (
                <SeletorEvidencias
                  ligado={evidenciasVisiveis}
                  aoAlternar={alternarTodasEvidencias}
                />
              )}
              {secoes.length === 0 && (
                <p className="muted">Nenhuma afirmação registrada ainda.</p>
              )}
              {secoes.map(([secao, afirmacoes]) => {
                const mostradas = afirmacoes.filter((a) =>
                  passaNoFiltro(a, filtro),
                );
                return (
                  <div key={secao} className="secao">
                    <h4 id={`secao-${secao}`} tabIndex={-1}>
                      {titulo(secao)}
                    </h4>
                    {diagramas
                      .filter((d) => d.secao === secao)
                      .map((d) => (
                        <DiagramaAoVivo
                          key={d.numero}
                          diagrama={d}
                          mermaid={vistos.get(d.numero)?.mermaid}
                          achados={vistos.get(d.numero)?.achados}
                          perguntas={analise.perguntas}
                          revisao={rev}
                          evidenciaAberta={evidenciaAberta}
                          aoAbrirEvidencia={(e) =>
                            aoAbrirEvidencia(e, `diagrama-${d.numero}`)
                          }
                          trecho={trecho}
                          trechoAberto={origemTrecho === `diagrama-${d.numero}`}
                        />
                      ))}
                    {afirmacoes.length > 0 && mostradas.length === 0 && (
                      <p className="muted">
                        Nenhuma afirmação desta seção neste filtro.
                      </p>
                    )}
                    {mostradas.length > 0 && (
                      <ol className="afirmacoes cartoes">
                        {mostradas.map((a) => (
                          <CartaoAfirmacao
                            key={a.numero}
                            afirmacao={a}
                            revisao={rev}
                            atual={a.numero === atual}
                            comando={comando}
                            evidenciaAberta={evidenciaAberta}
                            aoAbrirEvidencia={(e) =>
                              aoAbrirEvidencia(e, `afirmacao-${a.numero}`)
                            }
                            evidenciasAbertas={codigoVisivel(
                              `afirmacao-${a.numero}`,
                            )}
                            aoAlternarEvidencias={() =>
                              alternarCodigo(`afirmacao-${a.numero}`)
                            }
                            aoFocar={() => setAtual(a.numero)}
                            trecho={
                              origemTrecho === `afirmacao-${a.numero}`
                                ? trecho
                                : undefined
                            }
                          />
                        ))}
                      </ol>
                    )}
                  </div>
                );
              })}
            </section>
          )}

          <section id={ID_PERGUNTAS} aria-label="Perguntas em aberto">
            <h3 tabIndex={-1} id={`${ID_PERGUNTAS}-titulo`}>
              Perguntas para o negócio
            </h3>
            {perguntas.length === 0 ? (
              <p className="muted">Nenhuma pergunta registrada.</p>
            ) : (
              <ol className="perguntas cartoes">
                {perguntas.map((p) => (
                  <CartaoPergunta
                    key={p.numero}
                    pergunta={p}
                    revisao={rev}
                    diagramas={diagramas}
                  />
                ))}
              </ol>
            )}
            {rev && <EnviarRespostas perguntas={perguntas} revisao={rev} />}
          </section>

          {comPropostas && (
            <section id={ID_PROPOSTAS} aria-label="Propostas para a base">
              <h3 tabIndex={-1} id={`${ID_PROPOSTAS}-titulo`}>
                Propostas para a base
              </h3>
              {analise.termos.length + sugestoes.length === 0 ? (
                <p className="muted">Nenhuma proposta registrada.</p>
              ) : (
                <ol className="propostas cartoes">
                  {analise.termos.map((t) => (
                    <li
                      key={`termo-${t.numero}`}
                      className={`termo cartao estado-${t.estado}`}
                    >
                      <div className="cartao-cabeca">
                        <span className="badge">termo do glossário</span>
                        {t.estado !== "proposto" && (
                          <span
                            className="badge"
                            data-tone={t.estado === "aceito" ? "done" : "error"}
                          >
                            {t.estado}
                          </span>
                        )}
                      </div>
                      <p>
                        <strong>{t.termo}</strong>: {t.definicao}
                      </p>
                      {t.definicao_original && (
                        <span
                          className="muted editada"
                          title={`Definição do agente: ${t.definicao_original}`}
                        >
                          (definição editada pelo analista)
                        </span>
                      )}
                      {rev && <AcoesTermo termo={t} revisao={rev} />}
                    </li>
                  ))}
                  {sugestoes.map((s) => (
                    <li
                      key={`sugestao-${s.numero}`}
                      className="sugestao cartao"
                    >
                      <div className="cartao-cabeca">
                        <span className="badge">alteração sugerida</span>
                        {s.estado === "aceita" && (
                          <span className="badge" data-tone="done">
                            aceita
                          </span>
                        )}
                      </div>
                      <p>
                        <strong>{s.titulo || s.arquivo}</strong>
                        {s.id && <span className="muted"> ({s.id})</span>}
                      </p>
                      <p>{s.motivo}</p>
                      {s.evidencia && (
                        <BotaoCodigo
                          quantidade={1}
                          aberto={codigoVisivel(`sugestao-${s.numero}`)}
                          aoAlternar={() =>
                            alternarCodigo(`sugestao-${s.numero}`)
                          }
                        />
                      )}
                      {s.evidencia && codigoVisivel(`sugestao-${s.numero}`) && (
                        <button
                          type="button"
                          className="btn btn-sm evidencia"
                          aria-pressed={
                            evidenciaAberta === chaveEvidencia(s.evidencia)
                          }
                          onClick={() =>
                            s.evidencia &&
                            aoAbrirEvidencia(
                              s.evidencia,
                              `sugestao-${s.numero}`,
                            )
                          }
                        >
                          {chaveEvidencia(s.evidencia)}
                        </button>
                      )}
                      {origemTrecho === `sugestao-${s.numero}` && trecho}
                      {rev && s.estado === "proposta" && (
                        <AcoesAceite
                          tipo="sugestao"
                          numero={s.numero}
                          rotulo={`da sugestão ${s.numero}`}
                          revisao={rev}
                        />
                      )}
                    </li>
                  ))}
                </ol>
              )}
            </section>
          )}

          <p className="muted documento-caminho">
            Documento na base: <code>{vista.documento}</code>
          </p>
        </div>
      </div>
    </article>
  );
}

/** Cartão de uma pergunta para o negócio (#184), com a resposta no próprio cartão. */
function CartaoPergunta({
  pergunta: p,
  revisao,
  diagramas = [],
}: {
  pergunta: Pergunta;
  revisao?: Revisao;
  diagramas?: Diagrama[];
}) {
  const origem = p.origem;
  const diagrama =
    origem?.tipo === "aresta"
      ? diagramas.find((d) => d.numero === origem.diagrama)
      : undefined;
  const rotulo = (c: string) =>
    diagrama?.nos.find((n) => n.chave === c)?.rotulo ?? c;
  return (
    <li
      className={`pergunta cartao estado-${p.estado}`}
      data-testid={`pergunta-${p.numero}`}
    >
      <div className="cartao-cabeca">
        <span className="cartao-numero" title={`Pergunta ${p.numero}`}>
          {p.numero}
        </span>
        {p.para && (
          <span className="para-quem">
            Para: <strong>{p.para.nome}</strong>
            {p.para.papel && <span className="muted"> ({p.para.papel})</span>}
          </span>
        )}
        {p.estado === "aberta" && (
          <span className="badge" data-tone="attention">
            aguardando resposta
          </span>
        )}
        {p.estado === "descartada" && <span className="badge">descartada</span>}
      </div>
      <p className="texto-pergunta">{p.texto}</p>
      {p.origem?.tipo === "achado-diagrama" && (
        <span className="muted origem-pergunta">
          de um achado do D-{p.origem.diagrama}
          {(p.evidencias ?? []).map((e) => ` · ${chaveEvidencia(e)}`).join("")}
        </span>
      )}
      {origem?.tipo === "aresta" && (
        <span className="muted origem-pergunta">
          {diagrama?.tipo === "dados" ? "da relação" : "da transição"}{" "}
          {rotulo(origem.de)} → {rotulo(origem.para)} do D-{origem.diagrama}
          {(p.evidencias ?? []).map((e) => ` · ${chaveEvidencia(e)}`).join("")}
        </span>
      )}
      {p.estado === "respondida" && (
        <p className="muted resposta-pergunta">
          Resposta: {p.resposta}
          {p.resposta_enviada && " (enviada ao agente)"}
        </p>
      )}
      {revisao && <AcoesPergunta pergunta={p} revisao={revisao} />}
    </li>
  );
}

/**
 * Afirmações por seção, na ordem em que cada seção apareceu; `outras` são seções que
 * têm só diagramas (entram com a lista de afirmações vazia).
 */
function agrupar(
  afirmacoes: Afirmacao[],
  outras: string[] = [],
  secoes?: SecaoDocumento[],
): [string, Afirmacao[]][] {
  const grupos = new Map<string, Afirmacao[]>();
  for (const s of outras) grupos.set(s, []);
  for (const a of [...afirmacoes].sort((x, y) => x.numero - y.numero)) {
    const lista = grupos.get(a.secao) ?? [];
    lista.push(a);
    grupos.set(a.secao, lista);
  }
  // Na ordem do template, como no `.md`.
  return ordenarSecoes([...grupos], secoes);
}
