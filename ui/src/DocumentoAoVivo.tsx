import { useState, type ReactNode } from "react";
import type {
  Afirmacao,
  Confianca,
  EstadoAnalise,
  EstadoSessao,
  Evidencia,
  Pergunta,
  SessaoVista,
  VistaAnalise,
} from "./analise";
import {
  AcoesAceite,
  AcoesAfirmacao,
  AcoesPergunta,
  EnviarRespostas,
  AcoesTermo,
  type Revisao,
} from "./AcoesRevisao";
import { HistoriasAoVivo } from "./HistoriasAoVivo";
import { TextoAfirmacao } from "./TextoAfirmacao";
import { Icon } from "./shell/icons";

export const ROTULO_CONFIANCA: Record<Confianca, string> = {
  confirmado: "confirmado",
  inferido: "inferido",
  a_confirmar: "a confirmar",
};

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

/**
 * Seções do documento "pergunta ao sistema" (`cba_docs::MODELOS`), na ordem do
 * template; seções de outros fluxos aparecem depois, com o nome do marcador.
 */
const SECOES_CONHECIDAS: [string, string][] = [
  ["resposta", "Resposta"],
  ["regras_encontradas", "Regras encontradas"],
  ["exemplo", "Exemplo"],
  ["contexto_no_codigo", "O que a base e o código dizem"],
];
const TITULO_SECAO = Object.fromEntries(SECOES_CONHECIDAS) as Record<string, string>;
const ORDEM_SECAO = new Map(SECOES_CONHECIDAS.map(([s], i) => [s, i]));

const ESTADO_AFIRMACAO: Partial<Record<Afirmacao["estado"], string>> = {
  confirmada: "confirmada pelo analista",
  incorreta: "marcada como incorreta",
  aprofundar: "aprofundamento pedido",
  "a-revalidar": "a revalidar: o código mudou",
};

export function SeloSessao({ sessao }: { sessao: SessaoVista | null }) {
  if (!sessao) {
    return (
      <p className="selo-sessao sem-sessao" role="status" aria-label="Estado da sessão">
        Nenhuma sessão ligada ainda: ela aparece quando o agente chama a primeira ferramenta.
      </p>
    );
  }
  return (
    <p
      className="selo-sessao"
      role="status"
      aria-label="Estado da sessão"
      data-estado={sessao.estado}
    >
      {ROTULO_SESSAO[sessao.estado]}
    </p>
  );
}

export function chaveEvidencia(e: Evidencia): string {
  return `${e.arquivo}:${e.linhas}`;
}

/** Onde uma evidência foi escolhida: o trecho abre logo abaixo dela (#130). */
export type OrigemEvidencia = `afirmacao-${number}` | `sugestao-${number}`;

interface Props {
  vista: VistaAnalise;
  /** Evidência aberta (destacada na lista). */
  evidenciaAberta?: string | null;
  aoAbrirEvidencia: (e: Evidencia, origem: OrigemEvidencia) => void;
  /** Trecho da evidência aberta, mostrado abaixo do item em `origemTrecho`. */
  trecho?: ReactNode;
  origemTrecho?: OrigemEvidencia | null;
  /** Ações de revisão do analista (#37); sem elas, ou com a análise aprovada, só leitura. */
  revisao?: Revisao;
  /** Sem o cabeçalho (id, título, estado e sessão), quando a barra da tela já os mostra. */
  semCabecalho?: boolean;
}

/** Ícone que mostra ou esconde as evidências no código de uma afirmação ou sugestão. */
function BotaoCodigo({
  quantidade,
  aberto,
  aoAlternar,
}: {
  quantidade: number;
  aberto: boolean;
  aoAlternar: () => void;
}) {
  const rotulo = `${aberto ? "Ocultar" : "Ver"} ${
    quantidade === 1 ? "a evidência" : `as ${quantidade} evidências`
  } no código`;
  return (
    <button
      type="button"
      className="btn-icone btn-acao btn-codigo"
      aria-label={rotulo}
      title={rotulo}
      aria-expanded={aberto}
      onClick={aoAlternar}
    >
      <Icon name="code" />
      <span className="contador-codigo">{quantidade}</span>
    </button>
  );
}

/** Documento da análise montado ao vivo a partir do estado gravado. */
export function DocumentoAoVivo({
  vista,
  evidenciaAberta,
  aoAbrirEvidencia,
  trecho,
  origemTrecho,
  revisao,
  semCabecalho = false,
}: Props) {
  const { analise } = vista;
  const deHistorias = analise.fluxo === "historias";
  // Aprovada, a revisão fecha: as regras já foram para a base.
  const rev = analise.estado === "aprovada" ? undefined : revisao;
  const secoes = agrupar(analise.afirmacoes);
  const perguntas = [...analise.perguntas].sort(
    (a, b) => Number(a.estado !== "aberta") - Number(b.estado !== "aberta") || a.numero - b.numero,
  );
  const sugestoes = analise.sugestoes.filter((s) => s.estado !== "recusada");
  // As evidências ficam recolhidas: o analista abre o código só quando quer (#130).
  const [comCodigo, setComCodigo] = useState<ReadonlySet<OrigemEvidencia>>(new Set());
  const codigoVisivel = (o: OrigemEvidencia) => comCodigo.has(o) || origemTrecho === o;
  const alternarCodigo = (o: OrigemEvidencia) => {
    setComCodigo((atual) => {
      const novo = new Set(atual);
      if (codigoVisivel(o)) novo.delete(o);
      else novo.add(o);
      return novo;
    });
    // Fechar o código fecha também o trecho aberto dali.
    if (codigoVisivel(o) && origemTrecho === o && evidenciaAberta) {
      const ev = evidenciaDaOrigem(o);
      if (ev) aoAbrirEvidencia(ev, o);
    }
  };
  const evidenciaDaOrigem = (o: OrigemEvidencia): Evidencia | undefined => {
    const [tipo, n] = o.split("-");
    const numero = Number(n);
    if (tipo === "afirmacao")
      return analise.afirmacoes
        .find((a) => a.numero === numero)
        ?.evidencias.find((e) => chaveEvidencia(e) === evidenciaAberta);
    return analise.sugestoes.find((s) => s.numero === numero)?.evidencia ?? undefined;
  };

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

      {/* Na revisão, o texto revisado já aparece no parecer, com as marcas. */}
      {analise.fluxo !== "revisar" && (
        <section aria-label={deHistorias ? "Pedido" : "Pergunta"}>
          <h3>{deHistorias ? "Pedido" : "Pergunta"}</h3>
          <blockquote>{analise.entrada}</blockquote>
        </section>
      )}

      {deHistorias ? (
        <HistoriasAoVivo historias={analise.historias ?? []} origem={analise.origem} revisao={rev} />
      ) : (
      <section aria-label="Afirmações">
        <h3>Afirmações</h3>
        {secoes.length === 0 && <p className="muted">Nenhuma afirmação registrada ainda.</p>}
        {secoes.map(([secao, afirmacoes]) => (
          <div key={secao} className="secao">
            <h4>{TITULO_SECAO[secao] ?? secao}</h4>
            <ol className="afirmacoes">
              {afirmacoes.map((a) => (
                <li
                  key={a.numero}
                  className="afirmacao"
                  data-estado={a.estado}
                  data-pendente={(rev && a.estado === "registrada") || undefined}
                  data-testid={`afirmacao-${a.numero}`}
                >
                  <span className="badge" data-tone={a.confianca === "confirmado" ? "done" : undefined}>
                    {ROTULO_CONFIANCA[a.confianca]}
                  </span>{" "}
                  <TextoAfirmacao texto={a.texto} />
                  {rev && a.estado === "registrada" && (
                    <span className="estado-afirmacao attention-text"> (sem decisão)</span>
                  )}
                  {ESTADO_AFIRMACAO[a.estado] && (
                    <span className="estado-afirmacao"> ({ESTADO_AFIRMACAO[a.estado]})</span>
                  )}
                  {a.estado === "incorreta" && (
                    <span className="comentario muted"> {a.comentario}</span>
                  )}
                  {a.estado === "aprofundar" && a.pedido && (
                    <span className="comentario muted"> {a.pedido}</span>
                  )}
                  {a.texto_original && (
                    <span className="muted editada" title={`Texto do agente: ${a.texto_original}`}>
                      {" "}
                      (texto editado pelo analista)
                    </span>
                  )}
                  {(a.evidencias.length > 0 || rev) && (
                    <div className="rodape-afirmacao">
                      {a.evidencias.length > 0 && (
                        <BotaoCodigo
                          quantidade={a.evidencias.length}
                          aberto={codigoVisivel(`afirmacao-${a.numero}`)}
                          aoAlternar={() => alternarCodigo(`afirmacao-${a.numero}`)}
                        />
                      )}
                      {rev && <AcoesAfirmacao afirmacao={a} revisao={rev} />}
                    </div>
                  )}
                  {a.evidencias.length > 0 && codigoVisivel(`afirmacao-${a.numero}`) && (
                    <ul className="evidencias" aria-label="Evidências">
                      {a.evidencias.map((e) => {
                        const chave = chaveEvidencia(e);
                        return (
                          <li key={chave}>
                            <button
                              type="button"
                              className="btn btn-sm evidencia"
                              aria-pressed={evidenciaAberta === chave}
                              title={e.trecho}
                              onClick={() => aoAbrirEvidencia(e, `afirmacao-${a.numero}`)}
                            >
                              {chave}
                            </button>
                            {e.estado === "a-revalidar" && (
                              <span className="muted"> a revalidar</span>
                            )}
                          </li>
                        );
                      })}
                    </ul>
                  )}
                  {origemTrecho === `afirmacao-${a.numero}` && trecho}
                </li>
              ))}
            </ol>
          </div>
        ))}
      </section>
      )}

      <section aria-label="Perguntas em aberto">
        <h3>Perguntas</h3>
        {perguntas.length === 0 ? (
          <p className="muted">Nenhuma pergunta registrada.</p>
        ) : (
          <ol className="perguntas">
            {perguntas.map((p) => (
              <li key={p.numero} className={`pergunta estado-${p.estado}`}>
                {p.texto}
                <EstadoDaPergunta pergunta={p} />
                {rev && <AcoesPergunta pergunta={p} revisao={rev} />}
              </li>
            ))}
          </ol>
        )}
        {rev && <EnviarRespostas perguntas={perguntas} revisao={rev} />}
      </section>

      {!deHistorias && (
      <section aria-label="Sugestões de alteração">
        <h3>Sugestões de alteração</h3>
        {sugestoes.length === 0 ? (
          <p className="muted">Nenhuma sugestão registrada.</p>
        ) : (
          <ol className="sugestoes">
            {sugestoes.map((s) => (
              <li key={s.numero}>
                <strong>{s.titulo || s.arquivo}</strong>
                {s.id && <span className="muted"> ({s.id})</span>}
                {s.estado === "aceita" && <span className="muted"> (aceita)</span>}
                <p>{s.motivo}</p>
                {s.evidencia && (
                  <BotaoCodigo
                    quantidade={1}
                    aberto={codigoVisivel(`sugestao-${s.numero}`)}
                    aoAlternar={() => alternarCodigo(`sugestao-${s.numero}`)}
                  />
                )}
                {s.evidencia && codigoVisivel(`sugestao-${s.numero}`) && (
                  <button
                    type="button"
                    className="btn btn-sm evidencia"
                    aria-pressed={evidenciaAberta === chaveEvidencia(s.evidencia)}
                    onClick={() =>
                      s.evidencia && aoAbrirEvidencia(s.evidencia, `sugestao-${s.numero}`)
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

      {analise.termos.length > 0 && (
        <section aria-label="Termos propostos para o glossário">
          <h3>Termos para o glossário</h3>
          <ol className="termos">
            {analise.termos.map((t) => (
              <li key={t.numero} className={`termo estado-${t.estado}`}>
                <strong>{t.termo}</strong>: {t.definicao}
                {t.estado !== "proposto" && <span className="muted"> ({t.estado})</span>}
                {t.definicao_original && (
                  <span
                    className="muted editada"
                    title={`Definição do agente: ${t.definicao_original}`}
                  >
                    {" "}
                    (definição editada pelo analista)
                  </span>
                )}
                {rev && <AcoesTermo termo={t} revisao={rev} />}
              </li>
            ))}
          </ol>
        </section>
      )}

      <p className="muted documento-caminho">
        Documento na base: <code>{vista.documento}</code>
      </p>
    </article>
  );
}

function EstadoDaPergunta({ pergunta }: { pergunta: Pergunta }) {
  if (pergunta.estado === "respondida")
    return (
      <span className="muted">
        {" "}
        Resposta: {pergunta.resposta}
        {pergunta.resposta_enviada && " (enviada ao agente)"}
      </span>
    );
  if (pergunta.estado === "descartada") return <span className="muted"> (descartada)</span>;
  return null;
}

/** Afirmações por seção, na ordem em que cada seção apareceu. */
function agrupar(afirmacoes: Afirmacao[]): [string, Afirmacao[]][] {
  const grupos = new Map<string, Afirmacao[]>();
  for (const a of [...afirmacoes].sort((x, y) => x.numero - y.numero)) {
    const lista = grupos.get(a.secao) ?? [];
    lista.push(a);
    grupos.set(a.secao, lista);
  }
  const ordem = (s: string) => ORDEM_SECAO.get(s) ?? ORDEM_SECAO.size;
  return [...grupos].sort(([a], [b]) => ordem(a) - ordem(b));
}
