import { useCallback, useEffect, useMemo, useState } from "react";
import { mensagemDe } from "./analise";
import { Alerta, EstadoVazio, Secao, Selo, TextoRico, type TomSelo } from "./componentes";
import {
  acaoDoDestino,
  apiEntregaveisDemandaTauri,
  podeEnviar,
  ROTULO_SITUACAO,
  type ApiEntregaveisDemanda,
  type BaseVista,
  type EntregavelVisto,
  type FuncionalidadeVista,
  type PropostaVista,
  type Situacao,
  type TipoNaDemanda,
  type VistaEntregaveis,
} from "./entregaveisDaDemanda";
import { TerminalClaude } from "./TerminalClaude";
import type { ApiTerminal } from "./terminal";
import type { TelaTerminal } from "./telaXterm";
import "./entregaveisDemanda.css";

const TOM_SITUACAO: Record<Situacao, TomSelo> = {
  gerando: "andamento",
  rascunho: "atencao",
  revisado: "concluido",
  enviado: "concluido",
};

const CICLO: Record<TipoNaDemanda["ciclo"], string> = {
  vivo: "documento vivo",
  por_demanda: "por demanda",
};

/**
 * Seção "Entregáveis" do painel da demanda (#463): gerar o artefato da metodologia
 * a partir de uma análise validada, com a revisão final obrigatória.
 *
 * - **Base:** a análise validada, com as regras confirmadas, as decisões respondidas
 *   e as perguntas em aberto. Sem análise validada, a seção explica e não oferece a
 *   geração.
 * - **Tipos:** só os habilitados no projeto, com o padrão marcado. No tipo vivo, o
 *   analista escolhe a funcionalidade; sem documento, a demanda gera antes a v1.0.
 * - **Sessão:** "Gerar" abre o rascunho e o terminal do Claude com o prompt.
 * - **Entregáveis:** cada um como rascunho, com a faixa "Revisão final obrigatória"
 *   e o que conferir, o aviso de cobertura, "Mostrar evidências" e, no vivo, os
 *   trechos marcados da proposta. Enviar ou publicar fica desabilitado até "Marcar
 *   como revisado"; editar ou gerar de novo volta a rascunho.
 */
export function EntregaveisDemanda({
  projeto,
  demanda,
  api = apiEntregaveisDemandaTauri,
  atualizacao,
  abrirLink,
  apiTerminal,
  criarTela,
}: {
  projeto: string;
  demanda: string;
  api?: ApiEntregaveisDemanda;
  /** Muda quando o painel relê (o agente registrou, a base mudou): a seção relê. */
  atualizacao?: unknown;
  /** Abre a issue da demanda no navegador (comentário no rastreador). */
  abrirLink?: (projeto: string, link: string) => Promise<void>;
  /** Para testes. */
  apiTerminal?: ApiTerminal;
  criarTela?: () => TelaTerminal;
}) {
  const [vista, setVista] = useState<VistaEntregaveis | null>(null);
  const [falha, setFalha] = useState<string | null>(null);
  const [aviso, setAviso] = useState<string | null>(null);
  const [escolhida, setEscolhida] = useState<string | null>(null);
  const [sessao, setSessao] = useState<{ numero: number; prompt: string } | null>(null);

  const carregar = useCallback(() => {
    Promise.resolve()
      .then(() => api.vista(projeto, demanda))
      .then((v) => {
        if (v && Array.isArray(v.analises)) setVista(v);
      })
      .catch((e: unknown) => setFalha(mensagemDe(e)));
  }, [api, projeto, demanda]);

  useEffect(carregar, [carregar, atualizacao]);

  const agir = (acao: () => Promise<string | void>) => {
    setFalha(null);
    setAviso(null);
    acao()
      .then((texto) => {
        if (texto) setAviso(texto);
        carregar();
      })
      .catch((e: unknown) => setFalha(mensagemDe(e)));
  };

  if (!vista) {
    return (
      <Secao titulo="Entregáveis">
        {falha ? (
          <p className="error-text" role="alert">
            {falha}
          </p>
        ) : (
          <p className="muted">Carregando…</p>
        )}
      </Secao>
    );
  }

  const base =
    vista.analises.find((a) => a.analise === escolhida) ?? vista.analises[vista.analises.length - 1];

  const gerar = (tipo: string, funcionalidade: string | null, versaoInicial: boolean, analise = base?.analise) =>
    agir(async () => {
      if (!analise) return;
      const g = await api.gerar(projeto, demanda, { tipo, analise, funcionalidade, versaoInicial });
      setSessao({ numero: g.numero, prompt: g.prompt });
      return `E-${g.numero}: rascunho v${g.versao} aberto. O agente redige no terminal abaixo; a revisão final é sua.`;
    });

  return (
    <Secao titulo="Entregáveis" className="entregaveis-demanda">
      {falha && (
        <p className="error-text" role="alert">
          {falha}
        </p>
      )}
      {aviso && <p role="status">{aviso}</p>}

      {!base ? (
        <EstadoVazio>
          Sem análise validada: aprove uma análise de impacto, explicação ou pergunta ligada a esta
          demanda para gerar os entregáveis a partir dela.
        </EstadoVazio>
      ) : (
        <>
          <BaseDaAnalise
            analises={vista.analises}
            base={base}
            aoEscolher={setEscolhida}
          />
          <TiposDaDemanda
            tipos={vista.tipos}
            funcionalidades={vista.funcionalidades}
            aberta={vista.aberta}
            aoGerar={gerar}
          />
        </>
      )}

      {sessao && (
        <section className="entregaveis-sessao" aria-label="Sessão do agente">
          <div className="entregaveis-sessao-barra">
            <span>Sessão do agente: E-{sessao.numero}</span>
            <button type="button" className="btn" onClick={() => setSessao(null)}>
              Encerrar a sessão
            </button>
          </div>
          <div className="terminal-mapeamento">
            <TerminalClaude
              key={sessao.prompt + sessao.numero}
              projeto={projeto}
              pedido={{ promptInicial: sessao.prompt }}
              api={apiTerminal}
              criarTela={criarTela}
            />
          </div>
        </section>
      )}

      {vista.entregaveis.length > 0 && (
        <ul className="entregaveis-lista" aria-label="Entregáveis gerados">
          {vista.entregaveis.map((e) => (
            <CartaoEntregavel
              key={e.numero}
              e={e}
              aberta={vista.aberta}
              aoEditar={(texto) => agir(() => api.editar(projeto, demanda, e.numero, texto))}
              aoRevisar={() => agir(() => api.marcarRevisado(projeto, demanda, e.numero))}
              aoGerarDeNovo={() =>
                gerar(e.tipo, e.funcionalidade ?? null, e.finalidade === "versao_inicial", e.analise)
              }
              aoContinuar={() => setSessao({ numero: e.numero, prompt: e.prompt })}
              aoEnviar={() =>
                agir(async () => {
                  const acao = acaoDoDestino(e);
                  if (acao.tipo === "publicar") {
                    const onde = await api.publicar(projeto, demanda, e.numero);
                    return `E-${e.numero} publicado em ${onde}.`;
                  }
                  if (acao.tipo === "comentario") {
                    const c = await api.comentario(projeto, demanda, e.numero);
                    await api.copiar(c.texto);
                    if (c.link && abrirLink) await abrirLink(projeto, c.link);
                    return `Texto de E-${e.numero} copiado: cole como comentário na issue ${c.chave ?? ""}`.trim() + ".";
                  }
                  const destino = await api.escolherDestino(`${demanda}-E-${e.numero}-${e.tipo}`);
                  if (!destino) return;
                  const gravado = await api.exportar(projeto, demanda, e.numero, destino);
                  return `E-${e.numero} exportado em ${gravado}.`;
                })
              }
            />
          ))}
        </ul>
      )}
    </Secao>
  );
}

/** A análise validada de base: regras confirmadas, decisões respondidas, perguntas em aberto. */
function BaseDaAnalise({
  analises,
  base,
  aoEscolher,
}: {
  analises: BaseVista[];
  base: BaseVista;
  aoEscolher: (id: string) => void;
}) {
  return (
    <section className="entregaveis-base" aria-label="Análise validada de base">
      {analises.length > 1 ? (
        <label className="field">
          Análise validada de base
          <select value={base.analise} onChange={(ev) => aoEscolher(ev.target.value)}>
            {analises.map((a) => (
              <option key={a.analise} value={a.analise}>
                {a.analise} · {a.titulo}
              </option>
            ))}
          </select>
        </label>
      ) : (
        <p className="entregaveis-base-titulo">
          Análise validada de base: <span className="mono">{base.analise}</span> · {base.titulo}
        </p>
      )}
      <div className="entregaveis-base-colunas">
        <div>
          <h4>Regras confirmadas ({base.regras.length})</h4>
          {base.regras.length === 0 ? (
            <p className="muted">Nenhuma regra confirmada.</p>
          ) : (
            <ul>
              {base.regras.map((r) => (
                <li key={r.id}>
                  <span className="mono">{r.id}</span> {r.titulo}
                </li>
              ))}
            </ul>
          )}
        </div>
        <div>
          <h4>Decisões respondidas ({base.decisoes.length})</h4>
          {base.decisoes.length === 0 ? (
            <p className="muted">Nenhuma.</p>
          ) : (
            <ul>
              {base.decisoes.map((d) => (
                <li key={d.numero}>
                  {d.texto} <strong>→ {d.resposta}</strong>
                </li>
              ))}
            </ul>
          )}
        </div>
        <div>
          <h4>Perguntas em aberto ({base.perguntas.length})</h4>
          {base.perguntas.length === 0 ? (
            <p className="muted">Nenhuma.</p>
          ) : (
            <ul>
              {base.perguntas.map((p) => (
                <li key={p.numero}>{p.texto}</li>
              ))}
            </ul>
          )}
        </div>
      </div>
    </section>
  );
}

/** Os tipos habilitados no projeto, com o padrão primeiro, e o botão de gerar de cada um. */
function TiposDaDemanda({
  tipos,
  funcionalidades,
  aberta,
  aoGerar,
}: {
  tipos: TipoNaDemanda[];
  funcionalidades: FuncionalidadeVista[];
  aberta: boolean;
  aoGerar: (tipo: string, funcionalidade: string | null, versaoInicial: boolean) => void;
}) {
  const ordenados = useMemo(
    () => [...tipos].sort((a, b) => Number(b.padrao) - Number(a.padrao)),
    [tipos],
  );
  return (
    <section className="entregaveis-tipos" aria-label="Gerar entregável">
      <h4>Gerar entregável</h4>
      {!aberta && <p className="muted">Reabra a demanda para gerar um entregável.</p>}
      <ul>
        {ordenados.map((t) => (
          <LinhaDoTipo
            key={t.id}
            t={t}
            funcionalidades={funcionalidades}
            aberta={aberta}
            aoGerar={aoGerar}
          />
        ))}
      </ul>
    </section>
  );
}

function LinhaDoTipo({
  t,
  funcionalidades,
  aberta,
  aoGerar,
}: {
  t: TipoNaDemanda;
  funcionalidades: FuncionalidadeVista[];
  aberta: boolean;
  aoGerar: (tipo: string, funcionalidade: string | null, versaoInicial: boolean) => void;
}) {
  const vivo = t.ciclo === "vivo";
  const [funcionalidade, setFuncionalidade] = useState<string>(funcionalidades[0]?.id ?? "");
  const f = funcionalidades.find((x) => x.id === funcionalidade);
  const doc = f?.documentos.find((d) => d.tipo === t.id);
  const semDocumento = !doc?.versao;
  const bloqueio = t.indisponivel ?? (vivo && !f ? "Nenhuma funcionalidade no mapa para documentar." : null);
  const rotulo = !vivo
    ? `Gerar ${t.nome}`
    : semDocumento
      ? `Gerar a v1.0 de ${t.nome}`
      : `Gerar a proposta de versão de ${t.nome}`;
  return (
    <li className="entregaveis-tipo">
      <div className="entregaveis-tipo-nome">
        <strong>{t.nome}</strong> {t.padrao && <Selo tom="andamento">padrão</Selo>}{" "}
        <Selo>{CICLO[t.ciclo]}</Selo>
        <p className="muted">{t.descricao}</p>
      </div>
      {vivo && funcionalidades.length > 0 && (
        <div className="entregaveis-tipo-vivo">
          <label className="field">
            Funcionalidade
            <select value={funcionalidade} onChange={(ev) => setFuncionalidade(ev.target.value)}>
              {funcionalidades.map((x) => (
                <option key={x.id} value={x.id}>
                  {x.id} · {x.nome}
                  {x.daAnalise ? " (da análise)" : ""}
                </option>
              ))}
            </select>
          </label>
          {doc && (
            <p className="muted" role="note">
              {semDocumento
                ? `Sem documento: gere a v1.0 (engenharia reversa) antes da proposta da demanda.`
                : `${t.nome} v${doc.versao}, ${doc.estado}.`}
            </p>
          )}
        </div>
      )}
      <button
        type="button"
        className="btn"
        disabled={!aberta || !!bloqueio}
        title={bloqueio ?? undefined}
        onClick={() => aoGerar(t.id, vivo ? funcionalidade : null, vivo && semDocumento)}
      >
        {rotulo}
      </button>
      {bloqueio && <p className="muted">{bloqueio}</p>}
    </li>
  );
}

/** Trechos da proposta do documento vivo: o novo em destaque, o anterior riscado. */
function TrechosDaProposta({ p }: { p: PropostaVista }) {
  return (
    <div className="entregaveis-proposta" aria-label={`Proposta da versão ${p.versao}`}>
      <p className="muted">
        Proposta da versão {p.versao}
        {p.base ? ` sobre a ${p.base}` : ""}: {p.mudancas}
      </p>
      {p.trechos.map((t, i) => (
        <div key={i} className="entregaveis-trecho" data-marca={t.marca}>
          {t.marca === "alterado" && t.anterior && (
            <del aria-label="Texto anterior">{t.anterior}</del>
          )}
          {t.marca === "removido" ? (
            <del aria-label="Removido">{t.texto}</del>
          ) : t.marca === "mantido" ? (
            <span>{t.texto}</span>
          ) : (
            <ins aria-label={t.marca === "novo" ? "Novo" : "Alterado"}>{t.texto}</ins>
          )}
        </div>
      ))}
      {p.historico.length > 0 && (
        <details>
          <summary>Histórico de revisões ({p.historico.length})</summary>
          <table className="entregaveis-historico">
            <thead>
              <tr>
                <th>Versão</th>
                <th>Data</th>
                <th>Demanda</th>
                <th>Revisado por</th>
                <th>O que mudou</th>
              </tr>
            </thead>
            <tbody>
              {p.historico.map((r) => (
                <tr key={r.versao}>
                  <td>{r.versao}</td>
                  <td>{r.data}</td>
                  <td>{r.demanda ?? ""}</td>
                  <td>{r.revisor}</td>
                  <td>{r.mudancas}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </details>
      )}
    </div>
  );
}

function CartaoEntregavel({
  e,
  aberta,
  aoEditar,
  aoRevisar,
  aoGerarDeNovo,
  aoContinuar,
  aoEnviar,
}: {
  e: EntregavelVisto;
  aberta: boolean;
  aoEditar: (texto: string) => void;
  aoRevisar: () => void;
  aoGerarDeNovo: () => void;
  aoContinuar: () => void;
  aoEnviar: () => void;
}) {
  const [editando, setEditando] = useState<string | null>(null);
  const [evidencias, setEvidencias] = useState(false);
  const acao = acaoDoDestino(e);
  const liberado = podeEnviar(e);
  const gerando = e.situacao === "gerando";
  const titulo = `E-${e.numero} · ${e.nome}`;
  const anteriores = e.versoes.filter((v) => v !== e.versao);
  return (
    <li className="entregaveis-cartao" aria-label={titulo}>
      <header className="entregaveis-cartao-cabecalho">
        <strong>{titulo}</strong>
        <Selo tom={TOM_SITUACAO[e.situacao]}>{ROTULO_SITUACAO[e.situacao]}</Selo>
        <span className="muted">
          rascunho v{e.versao}
          {anteriores.length > 0 && ` (anteriores guardadas: ${anteriores.map((v) => `v${v}`).join(", ")})`}
          {" · "}de {e.analise}
          {e.funcionalidade && ` · ${e.funcionalidade}`}
          {e.finalidade === "versao_inicial" && " · versão inicial"}
        </span>
      </header>

      {!liberado && !gerando && (
        <div className="entregaveis-faixa-revisao" role="note" aria-label="Revisão final obrigatória">
          <strong>Revisão final obrigatória</strong>
          <span> · nada é enviado nem publicado antes de você marcar como revisado. Confira:</span>
          <ul>
            {e.conferir.map((c) => (
              <li key={c}>{c}</li>
            ))}
          </ul>
        </div>
      )}
      {liberado && e.revisadoPor && (
        <p className="muted">Revisado por {e.revisadoPor}.</p>
      )}

      {e.avisoCobertura && (
        <Alerta tom="atencao" titulo={e.avisoCobertura} role="note">
          Regra confirmada da análise sem item no entregável: cubra-a ou registre por que ficou de fora.
        </Alerta>
      )}

      {gerando ? (
        <p className="muted">Aguardando o agente registrar o conteúdo.</p>
      ) : editando !== null ? (
        <form
          aria-label={`Editar ${titulo}`}
          onSubmit={(ev) => {
            ev.preventDefault();
            aoEditar(editando);
            setEditando(null);
          }}
        >
          <textarea
            aria-label="Texto do entregável"
            rows={14}
            value={editando}
            onChange={(ev) => setEditando(ev.target.value)}
          />
          <div className="entregaveis-acoes">
            <button type="submit" className="btn btn-primary">
              Salvar a edição
            </button>
            <button type="button" className="btn" onClick={() => setEditando(null)}>
              Cancelar
            </button>
          </div>
        </form>
      ) : e.proposta ? (
        <TrechosDaProposta p={e.proposta} />
      ) : (
        <div className="entregaveis-texto">
          <TextoRico texto={e.texto} />
        </div>
      )}

      {!gerando && e.itens.length > 0 && (
        <div>
          <button
            type="button"
            className="link"
            aria-expanded={evidencias}
            onClick={() => setEvidencias((v) => !v)}
          >
            {evidencias ? "Ocultar evidências" : "Mostrar evidências"}
          </button>
          {evidencias && (
            <ul className="entregaveis-evidencias" aria-label="Evidências">
              {e.itens.map((i) => (
                <li key={i.numero}>
                  <span className="muted">Item {i.numero}</span>
                  <ul>
                    {i.regras.map((r) => (
                      <li key={r.id}>
                        <span className="mono">{r.id}</span> {r.titulo}
                        {r.evidencias.length === 0 ? (
                          <p className="muted">Regra sem evidência de código.</p>
                        ) : (
                          r.evidencias.map((ev) => (
                            <figure key={`${ev.arquivo}:${ev.inicio}`} className="entregaveis-trecho-codigo">
                              <figcaption className="mono">
                                {ev.arquivo}:{ev.inicio}
                                {ev.fim !== ev.inicio ? `-${ev.fim}` : ""}
                              </figcaption>
                              {ev.trecho ? (
                                <pre>{ev.trecho}</pre>
                              ) : (
                                <p className="muted">O arquivo não pôde ser lido agora.</p>
                              )}
                            </figure>
                          ))
                        )}
                      </li>
                    ))}
                  </ul>
                </li>
              ))}
            </ul>
          )}
        </div>
      )}

      {e.envios.length > 0 && (
        <ul className="entregaveis-envios muted" aria-label="Envios">
          {e.envios.map((v, i) => (
            <li key={i}>
              v{v.versao} → {v.onde} ({v.por})
            </li>
          ))}
        </ul>
      )}

      <div className="entregaveis-acoes">
        <button
          type="button"
          className="btn"
          disabled={gerando || editando !== null || (e.ciclo === "vivo" && !e.proposta)}
          title={
            e.ciclo === "vivo" && !e.proposta && !gerando
              ? "A versão já foi publicada: gere de novo para propor a seguinte."
              : undefined
          }
          onClick={() => setEditando(e.texto)}
        >
          Editar
        </button>
        <button type="button" className="btn" disabled={!aberta} onClick={aoGerarDeNovo}>
          Gerar de novo
        </button>
        {gerando && (
          <button type="button" className="btn" onClick={aoContinuar}>
            Abrir a sessão
          </button>
        )}
        <button
          type="button"
          className="btn"
          disabled={gerando || liberado}
          onClick={aoRevisar}
        >
          Marcar como revisado
        </button>
        <button
          type="button"
          className="btn btn-primary"
          disabled={!liberado}
          title={liberado ? undefined : "Marque como revisado antes de enviar ou publicar."}
          onClick={aoEnviar}
        >
          {acao.texto}
        </button>
      </div>
    </li>
  );
}
