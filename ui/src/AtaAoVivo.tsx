import { useCallback, useEffect, useRef, useState } from "react";
import { mensagemDe, type AnaliseCriada, type AvisoAnalise } from "./analise";
import { ROTULO_STATUS_ATA } from "./demandas";
import { AvisoTranscriptGuardado } from "./PainelDemanda";
import { BotoesProximoPasso } from "./ProximoPasso";
import {
  TIPOS_ITEM,
  TITULO_TIPO_ITEM,
  type ApiReuniao,
  type AtaVista,
  type CitacaoVista,
  type ConfirmacaoParticipante,
  type ParticipanteVista,
} from "./reunioes";

type Vigiar = (projeto: string, aoAviso: (a: AvisoAnalise) => void) => Promise<() => void>;

/**
 * Ata da reunião, relida a cada aviso da vigia sobre ela (`Mudanca::Ata`, a cada
 * `registrar_item_ata` ou `propor_papel` do agente; `Mudanca::Transcript`) ou a cada
 * reinício da vigia. Releituras pedidas durante outra viram uma só, logo depois.
 */
export function useAtaAoVivo(api: ApiReuniao, vigiar: Vigiar, projeto: string, reuniao: string) {
  const [ata, setAta] = useState<AtaVista | null>(null);
  const [erro, setErro] = useState<string | null>(null);
  const carregando = useRef(false);
  const pendente = useRef(false);
  const ativo = useRef(true);

  const carregar = useCallback(() => {
    if (carregando.current) {
      pendente.current = true;
      return;
    }
    carregando.current = true;
    api
      .ataReuniao(projeto, reuniao)
      .then((a) => {
        if (!ativo.current) return;
        setAta(a);
        setErro(null);
      })
      .catch((e: unknown) => ativo.current && setErro(mensagemDe(e)))
      .finally(() => {
        carregando.current = false;
        if (pendente.current && ativo.current) {
          pendente.current = false;
          carregar();
        }
      });
  }, [api, projeto, reuniao]);

  useEffect(() => {
    ativo.current = true;
    let parar: (() => void) | null = null;
    let desmontado = false;
    // Assina antes de ler, para nenhuma mudança cair entre a leitura e a assinatura.
    vigiar(projeto, (aviso) => {
      if (aviso.tipo === "reinicio") carregar();
      else if (
        aviso.analise === reuniao &&
        (aviso.mudanca === "ata" || aviso.mudanca === "transcript")
      )
        carregar();
    })
      .then((f) => {
        if (desmontado) f();
        else parar = f;
      })
      .catch(() => undefined)
      .finally(() => {
        if (!desmontado) carregar();
      });
    return () => {
      desmontado = true;
      ativo.current = false;
      parar?.();
    };
  }, [vigiar, projeto, reuniao, carregar]);

  return { ata, setAta, erro, recarregar: carregar };
}

/**
 * Ata ao vivo da análise de uma reunião (#141), como o documento ao vivo das
 * análises: os participantes, com a confirmação do papel, e os itens registrados
 * pelo agente, com as falas de origem. Na ata aprovada, cada próximo passo tem os
 * botões que abrem análises ligadas à demanda (#142).
 */
export function AtaAoVivo({
  api,
  vigiar,
  projeto,
  reuniao,
  aoCriarAnalise,
}: {
  api: ApiReuniao;
  vigiar: Vigiar;
  projeto: string;
  reuniao: string;
  /** Análise aberta por um próximo passo; sem ela, os botões não aparecem. */
  aoCriarAnalise?: (criada: AnaliseCriada) => void;
}) {
  const { ata, setAta, erro } = useAtaAoVivo(api, vigiar, projeto, reuniao);

  if (!ata) {
    return erro ? (
      <p className="error-text" role="alert">
        Não foi possível ler a ata da reunião {reuniao}: {erro}
      </p>
    ) : (
      <p className="muted">Carregando a ata…</p>
    );
  }

  const aprovada = ata.status === "aprovada";

  return (
    <article className="documento-ao-vivo ata-ao-vivo" aria-label="Ata da reunião">
      <header>
        <h2>Ata da reunião {ata.id}</h2>
        <p className="muted">
          Demanda {ata.demanda} · {ata.data} ·{" "}
          <span className="badge" data-tone={aprovada ? "done" : "attention"}>
            ata {ROTULO_STATUS_ATA[ata.status]}
          </span>{" "}
          · {ata.itens.length} item(ns) registrado(s)
        </p>
        {ata.transcript && <AvisoTranscriptGuardado guardado={ata.transcript} />}
      </header>
      {erro && (
        <p className="error-text" role="alert">
          {erro}
        </p>
      )}
      {ata.objetivo && (
        <section aria-label="Objetivo">
          <h3>Objetivo</h3>
          <p>{ata.objetivo}</p>
        </section>
      )}
      {ata.resumo && (
        <section aria-label="Resumo">
          <h3>Resumo</h3>
          <p>{ata.resumo}</p>
        </section>
      )}

      <Participantes
        key={ata.id}
        api={api}
        projeto={projeto}
        ata={ata}
        somenteLeitura={aprovada}
        aoConfirmar={setAta}
      />

      <section aria-label="Itens da ata">
        <h3>Itens</h3>
        {ata.itens.length === 0 && (
          <p className="muted">
            Nenhum item registrado ainda: eles aparecem aqui a cada registro do agente.
          </p>
        )}
        {TIPOS_ITEM.map((tipo) => {
          const itens = ata.itens.filter((i) => i.tipo === tipo);
          if (itens.length === 0) return null;
          return (
            <div key={tipo} className="secao">
              <h4>{TITULO_TIPO_ITEM[tipo]}</h4>
              <ol className="itens-ata">
                {itens.map((i) => (
                  <li key={i.id} data-testid={`item-${i.id}`}>
                    <span className="badge mono">{i.id}</span> <span>{i.texto}</span>
                    {i.responsavel && (
                      <span className="muted"> · responsável: {i.responsavel}</span>
                    )}
                    {i.prazo && <span className="muted"> · prazo: {i.prazo}</span>}
                    {i.referencias.length > 0 && (
                      <span className="muted"> · ligado a {i.referencias.join(", ")}</span>
                    )}
                    <Citacoes citacoes={i.citacoes} />
                    {aprovada && i.tipo === "proximo_passo" && aoCriarAnalise && (
                      <BotoesProximoPasso
                        api={api}
                        projeto={projeto}
                        reuniao={ata.id}
                        passo={i}
                        aoCriar={aoCriarAnalise}
                      />
                    )}
                    {i.evidencias.length > 0 && (
                      <ul className="evidencias" aria-label="Evidências">
                        {i.evidencias.map((e) => (
                          <li key={e}>
                            <code>{e}</code>
                          </li>
                        ))}
                      </ul>
                    )}
                  </li>
                ))}
              </ol>
            </div>
          );
        })}
      </section>
      {ata.documento && (
        <p className="muted documento-caminho">
          Ata na base: <code>{ata.documento}</code>
        </p>
      )}
    </article>
  );
}

function Citacoes({ citacoes }: { citacoes: CitacaoVista[] }) {
  if (citacoes.length === 0) return null;
  return (
    <ul className="citacoes-fala" aria-label="Falas de origem">
      {citacoes.map((c, n) => (
        <li key={n}>
          <span className="mono">{c.inicio}</span> {c.falante}: “{c.trecho}”{" "}
          <span className="muted" data-tone={c.status === "não confere" ? "attention" : undefined}>
            ({c.status})
          </span>
        </li>
      ))}
    </ul>
  );
}

/** O que o analista está escolhendo para cada participante. */
interface Rascunho {
  papel: string;
  /** Pessoa do catálogo escolhida, no casamento ambíguo. */
  pessoa: string | null;
  /** Nome do participante que é a mesma pessoa (este será juntado a ele). */
  mesmoQue: string | null;
}

function rascunhoDe(p: ParticipanteVista): Rascunho {
  return {
    // O confirmado; sem ele, o do catálogo e, por fim, o proposto pelo agente.
    papel: p.papel ?? p.catalogo?.papel ?? p.proposta?.papel ?? "",
    pessoa: p.pessoa,
    mesmoQue: null,
  };
}

/** Destino final de uma junção (A → B → C vira A → C), sem laços. */
function destino(nome: string, rascunhos: Record<string, Rascunho>): string {
  const vistos = new Set<string>();
  let atual = nome;
  while (rascunhos[atual]?.mesmoQue && !vistos.has(atual)) {
    vistos.add(atual);
    atual = rascunhos[atual].mesmoQue!;
  }
  return atual;
}

/** Monta a confirmação: os participantes que ficam, com os nomes juntados a eles. */
export function montarConfirmacoes(
  participantes: ParticipanteVista[],
  rascunhos: Record<string, Rascunho>,
): ConfirmacaoParticipante[] {
  const ficam = participantes.filter((p) => destino(p.nome, rascunhos) === p.nome);
  return ficam.map((p) => {
    const r = rascunhos[p.nome] ?? rascunhoDe(p);
    return {
      nome: p.nome,
      papel: r.papel.trim() || null,
      pessoa: r.pessoa,
      mesmos: participantes
        .filter((o) => o.nome !== p.nome && destino(o.nome, rascunhos) === p.nome)
        .map((o) => o.nome),
    };
  });
}

function Participantes({
  api,
  projeto,
  ata,
  somenteLeitura,
  aoConfirmar,
}: {
  api: ApiReuniao;
  projeto: string;
  ata: AtaVista;
  somenteLeitura: boolean;
  aoConfirmar: (a: AtaVista) => void;
}) {
  const [rascunhos, setRascunhos] = useState<Record<string, Rascunho>>({});
  const [enviando, setEnviando] = useState(false);
  const [falha, setFalha] = useState<string | null>(null);
  const [feito, setFeito] = useState<string | null>(null);
  const idLista = `papeis-${ata.id}`;

  const rascunho = (p: ParticipanteVista) => rascunhos[p.nome] ?? rascunhoDe(p);
  const mudar = (p: ParticipanteVista, m: Partial<Rascunho>) => {
    setFeito(null);
    setRascunhos((r) => ({ ...r, [p.nome]: { ...rascunho(p), ...m } }));
  };

  const confirmar = () => {
    setEnviando(true);
    setFalha(null);
    setFeito(null);
    const todos: Record<string, Rascunho> = Object.fromEntries(
      ata.participantes.map((p) => [p.nome, rascunho(p)]),
    );
    api
      .confirmarParticipantes(projeto, ata.id, montarConfirmacoes(ata.participantes, todos))
      .then((nova) => {
        setRascunhos({});
        setFeito("Participantes confirmados: o catálogo de pessoas foi atualizado.");
        aoConfirmar(nova);
      })
      .catch((e: unknown) => setFalha(mensagemDe(e)))
      .finally(() => setEnviando(false));
  };

  return (
    <section aria-label="Participantes" className="participantes-reuniao">
      <h3>Participantes</h3>
      {ata.participantes.length === 0 && (
        <p className="muted">Nenhum participante identificado no transcript.</p>
      )}
      <datalist id={idLista}>
        {ata.papeis.map((p) => (
          <option key={p.nome} value={p.nome}>
            {p.descricao ?? undefined}
          </option>
        ))}
      </datalist>
      <ul className="lista-participantes">
        {ata.participantes.map((p) => {
          const r = rascunho(p);
          return (
            <li key={p.nome} data-testid={`participante-${p.nome}`}>
              <strong>{p.nome}</strong>{" "}
              {p.papel ? (
                <span className="badge" data-tone="done">
                  {p.papel}
                </span>
              ) : (
                <span className="badge" data-tone="attention">
                  papel a confirmar
                </span>
              )}
              <p className="muted">
                Catálogo:{" "}
                {p.catalogo
                  ? p.catalogo.papel
                    ? `${p.catalogo.pessoa.nome}, ${p.catalogo.papel}${
                        p.catalogo.reuniao ? ` (confirmado em ${p.catalogo.reuniao})` : ""
                      }`
                    : `${p.catalogo.pessoa.nome}, sem papel confirmado`
                  : p.candidatos.length > 0
                    ? "mais de uma pessoa com esse nome"
                    : "pessoa nova"}
              </p>
              {p.proposta && (
                <div className="proposta-agente">
                  <p>
                    Agente propõe: <strong>{p.proposta.papel}</strong>
                    {p.proposta.justificativa && (
                      <span className="muted"> · {p.proposta.justificativa}</span>
                    )}
                  </p>
                  <Citacoes citacoes={p.proposta.citacoes} />
                </div>
              )}
              {!somenteLeitura && (
                <div className="confirmacao-participante">
                  <label>
                    Papel de {p.nome}
                    <input
                      list={idLista}
                      value={r.papel}
                      disabled={r.mesmoQue !== null}
                      onChange={(e) => mudar(p, { papel: e.target.value })}
                      placeholder="PO, gerente, usuário-chave ou outro"
                    />
                  </label>
                  {p.candidatos.length > 0 && (
                    <label>
                      Pessoa do catálogo ({p.nome})
                      <select
                        value={r.pessoa ?? ""}
                        onChange={(e) => mudar(p, { pessoa: e.target.value || null })}
                      >
                        <option value="">Escolha</option>
                        {p.candidatos.map((c) => (
                          <option key={c.slug} value={c.slug}>
                            {c.nome} ({c.slug})
                          </option>
                        ))}
                      </select>
                    </label>
                  )}
                  {ata.participantes.length > 1 && (
                    <label>
                      É a mesma pessoa que ({p.nome})
                      <select
                        value={r.mesmoQue ?? ""}
                        onChange={(e) => mudar(p, { mesmoQue: e.target.value || null })}
                      >
                        <option value="">Ninguém: é outra pessoa</option>
                        {ata.participantes
                          .filter((o) => o.nome !== p.nome)
                          .map((o) => (
                            <option key={o.nome} value={o.nome}>
                              {o.nome}
                            </option>
                          ))}
                      </select>
                    </label>
                  )}
                </div>
              )}
            </li>
          );
        })}
      </ul>
      {!somenteLeitura && ata.participantes.length > 0 && (
        <div className="acoes">
          <button type="button" className="btn btn-primary" disabled={enviando} onClick={confirmar}>
            Confirmar os participantes
          </button>
        </div>
      )}
      {feito && (
        <p className="muted" role="status">
          {feito}
        </p>
      )}
      {falha && (
        <p className="error-text" role="alert">
          {falha}
        </p>
      )}
    </section>
  );
}
