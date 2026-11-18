import { useEffect, useState } from "react";
import { contar } from "./formatacao";
import {
  NOME_MOTIVO,
  NOME_TIPO_ENTRADA,
  type EscopoMapa,
  type Funcionalidade,
  type PontoJustificado,
  type SituacaoFuncionalidade,
} from "./analise";
import type { Revisao } from "./AcoesRevisao";
import type { DecisaoRevisao } from "./revisao";
import type { Completude } from "./funcionalidades";

const SITUACAO: Record<SituacaoFuncionalidade, string> = {
  pendente: "sem decisão",
  aceita: "aceita",
  descartada: "descartada",
  juntada: "juntada",
};

function situacao(f: Funcionalidade): SituacaoFuncionalidade {
  return f.situacao ?? "pendente";
}

function aberta(f: Funcionalidade): boolean {
  const s = situacao(f);
  return s === "pendente" || s === "aceita";
}

export function descricaoDoEscopo(escopo?: EscopoMapa | null): string {
  const pasta = escopo?.pasta;
  const area = escopo?.area;
  if (pasta && area) return `a pasta ${pasta}, na área ${area}`;
  if (pasta) return `a pasta ${pasta}`;
  if (area) return `a área ${area}`;
  return "o projeto inteiro";
}

type Form = "editar" | "juntar" | "dividir" | null;

function FormEditar({
  f,
  ocupado,
  aoSalvar,
  aoCancelar,
}: {
  f: Funcionalidade;
  ocupado: boolean;
  aoSalvar: (d: { nome: string; area: string; descricao: string }) => void;
  aoCancelar: () => void;
}) {
  const [nome, setNome] = useState(f.nome);
  const [area, setArea] = useState(f.area_nome);
  const [descricao, setDescricao] = useState(f.descricao);
  const valido = nome.trim() && area.trim() && descricao.trim();
  return (
    <form
      className="form-funcionalidade"
      aria-label={`Editar ${f.id}`}
      onSubmit={(e) => {
        e.preventDefault();
        if (valido) aoSalvar({ nome: nome.trim(), area: area.trim(), descricao: descricao.trim() });
      }}
    >
      <label>
        Nome
        <input value={nome} onChange={(e) => setNome(e.target.value)} />
      </label>
      <label>
        Área
        <input value={area} onChange={(e) => setArea(e.target.value)} />
      </label>
      <label>
        Descrição
        <textarea rows={2} value={descricao} onChange={(e) => setDescricao(e.target.value)} />
      </label>
      <div className="acoes">
        <button type="submit" className="btn btn-primary" disabled={ocupado || !valido}>
          Salvar
        </button>
        <button type="button" className="btn" onClick={aoCancelar}>
          Cancelar
        </button>
      </div>
    </form>
  );
}

function FormJuntar({
  f,
  outras,
  ocupado,
  aoJuntar,
  aoCancelar,
}: {
  f: Funcionalidade;
  outras: Funcionalidade[];
  ocupado: boolean;
  aoJuntar: (destino: string) => void;
  aoCancelar: () => void;
}) {
  const [destino, setDestino] = useState(outras[0]?.id ?? "");
  return (
    <form
      className="form-funcionalidade"
      aria-label={`Juntar ${f.id}`}
      onSubmit={(e) => {
        e.preventDefault();
        if (destino) aoJuntar(destino);
      }}
    >
      <label>
        Juntar {f.id} em
        <select value={destino} onChange={(e) => setDestino(e.target.value)}>
          {outras.map((o) => (
            <option key={o.id} value={o.id}>
              {o.id} · {o.nome}
            </option>
          ))}
        </select>
      </label>
      <p className="muted">
        Os pontos de entrada, os atores e os dados de {f.id} passam para a escolhida, e {f.id} sai do
        catálogo.
      </p>
      <div className="acoes">
        <button type="submit" className="btn btn-primary" disabled={ocupado || !destino}>
          Juntar
        </button>
        <button type="button" className="btn" onClick={aoCancelar}>
          Cancelar
        </button>
      </div>
    </form>
  );
}

function FormDividir({
  f,
  ocupado,
  aoDividir,
  aoCancelar,
}: {
  f: Funcionalidade;
  ocupado: boolean;
  aoDividir: (d: { pontos: number[]; nome: string; descricao: string }) => void;
  aoCancelar: () => void;
}) {
  const [escolhidos, setEscolhidos] = useState<ReadonlySet<number>>(new Set());
  const [nome, setNome] = useState("");
  const [descricao, setDescricao] = useState("");
  const valido =
    escolhidos.size > 0 && escolhidos.size < f.pontos.length && nome.trim() && descricao.trim();
  return (
    <form
      className="form-funcionalidade"
      aria-label={`Dividir ${f.id}`}
      onSubmit={(e) => {
        e.preventDefault();
        if (valido)
          aoDividir({
            pontos: [...escolhidos].sort((a, b) => a - b),
            nome: nome.trim(),
            descricao: descricao.trim(),
          });
      }}
    >
      <fieldset>
        <legend>Pontos de entrada que vão para a nova funcionalidade</legend>
        {f.pontos.map((p, i) => (
          <label key={p.chave ?? `${p.nome}-${i}`} className="opcao">
            <input
              type="checkbox"
              checked={escolhidos.has(i)}
              onChange={() =>
                setEscolhidos((atual) => {
                  const novo = new Set(atual);
                  if (novo.has(i)) novo.delete(i);
                  else novo.add(i);
                  return novo;
                })
              }
            />
            {p.nome}
          </label>
        ))}
      </fieldset>
      <label>
        Nome da nova
        <input value={nome} onChange={(e) => setNome(e.target.value)} />
      </label>
      <label>
        Descrição da nova
        <textarea rows={2} value={descricao} onChange={(e) => setDescricao(e.target.value)} />
      </label>
      {escolhidos.size === f.pontos.length && (
        <p className="muted">Deixe ao menos um ponto de entrada em {f.id}.</p>
      )}
      <div className="acoes">
        <button type="submit" className="btn btn-primary" disabled={ocupado || !valido}>
          Dividir
        </button>
        <button type="button" className="btn" onClick={aoCancelar}>
          Cancelar
        </button>
      </div>
    </form>
  );
}

function CartaoFuncionalidade({
  f,
  outras,
  revisao,
}: {
  f: Funcionalidade;
  outras: Funcionalidade[];
  revisao?: Revisao;
}) {
  const [form, setForm] = useState<Form>(null);
  const ocupado = revisao?.ocupado ?? true;
  const s = situacao(f);
  const decidir = (d: DecisaoRevisao) =>
    revisao
      ?.aoDecidir(d)
      .then(() => setForm(null))
      .catch(() => undefined);
  return (
    <section className={`funcionalidade situacao-${s}`} aria-label={`${f.id}: ${f.nome}`}>
      <header className="historia-cabecalho">
        <span className="painel-id">{f.id}</span>
        <strong>{f.nome}</strong>
        {f.do_catalogo && <span className="chip">já no catálogo</span>}
        <span className="badge" data-tone={s === "aceita" ? "done" : undefined}>
          {s === "juntada" && f.juntada_em ? `juntada em ${f.juntada_em}` : SITUACAO[s]}
        </span>
      </header>
      {form === "editar" && revisao ? (
        <FormEditar
          f={f}
          ocupado={ocupado}
          aoCancelar={() => setForm(null)}
          aoSalvar={(d) => decidir({ tipo: "editar-funcionalidade", funcionalidade: f.id, ...d })}
        />
      ) : (
        <p>
          {f.descricao}
          {f.editada && <span className="muted editada"> (editada pelo analista)</span>}
        </p>
      )}
      {f.atores && f.atores.length > 0 && (
        <p className="muted">
          <strong>Atores:</strong> {f.atores.join(", ")}
        </p>
      )}
      <ul className="pontos-entrada" aria-label={`Pontos de entrada de ${f.id}`}>
        {f.pontos.map((p, i) => (
          <li key={p.chave ?? `${p.nome}-${i}`}>
            <code>{p.nome}</code>
            {p.tipo && <span className="muted"> · {NOME_TIPO_ENTRADA[p.tipo]}</span>}
            <span className="muted">
              {" "}
              · {p.evidencia.arquivo}:{p.evidencia.linhas}
            </span>
            {!p.chave && <span className="chip">fora da lista do mapa</span>}
          </li>
        ))}
      </ul>
      {((f.dados_lidos?.length ?? 0) > 0 || (f.dados_gravados?.length ?? 0) > 0) && (
        <p className="muted">
          {(f.dados_lidos?.length ?? 0) > 0 && (
            <>
              <strong>Lê:</strong> {f.dados_lidos!.join(", ")}{" "}
            </>
          )}
          {(f.dados_gravados?.length ?? 0) > 0 && (
            <>
              <strong>Grava:</strong> {f.dados_gravados!.join(", ")}
            </>
          )}
        </p>
      )}
      {(f.relacionadas?.length ?? 0) > 0 && (
        <p className="muted">
          <strong>Relacionadas:</strong> {f.relacionadas!.join(", ")}
        </p>
      )}
      {form === "juntar" && revisao && (
        <FormJuntar
          f={f}
          outras={outras}
          ocupado={ocupado}
          aoCancelar={() => setForm(null)}
          aoJuntar={(destino) =>
            decidir({ tipo: "juntar-funcionalidades", origem: f.id, destino })
          }
        />
      )}
      {form === "dividir" && revisao && (
        <FormDividir
          f={f}
          ocupado={ocupado}
          aoCancelar={() => setForm(null)}
          aoDividir={(d) => decidir({ tipo: "dividir-funcionalidade", funcionalidade: f.id, ...d })}
        />
      )}
      {revisao && s !== "juntada" && form === null && (
        <div className="acoes-revisao" role="group" aria-label={`Revisão de ${f.id}`}>
          {s === "pendente" && (
            <button
              type="button"
              className="btn"
              disabled={ocupado}
              onClick={() =>
                decidir({ tipo: "funcionalidade", funcionalidade: f.id, decisao: "aceitar" })
              }
            >
              Aceitar
            </button>
          )}
          {aberta(f) && (
            <>
              <button
                type="button"
                className="btn"
                disabled={ocupado}
                onClick={() => setForm("editar")}
              >
                Editar
              </button>
              {outras.length > 0 && (
                <button
                  type="button"
                  className="btn"
                  disabled={ocupado}
                  onClick={() => setForm("juntar")}
                >
                  Juntar
                </button>
              )}
              {f.pontos.length > 1 && (
                <button
                  type="button"
                  className="btn"
                  disabled={ocupado}
                  onClick={() => setForm("dividir")}
                >
                  Dividir
                </button>
              )}
            </>
          )}
          {s !== "descartada" && (
            <button
              type="button"
              className="btn"
              disabled={ocupado}
              onClick={() =>
                decidir({ tipo: "funcionalidade", funcionalidade: f.id, decisao: "descartar" })
              }
            >
              Descartar
            </button>
          )}
          {s !== "pendente" && (
            <button
              type="button"
              className="btn"
              disabled={ocupado}
              onClick={() =>
                decidir({ tipo: "funcionalidade", funcionalidade: f.id, decisao: "desfazer" })
              }
            >
              Desfazer
            </button>
          )}
        </div>
      )}
    </section>
  );
}

/**
 * Mapa de funcionalidades em revisão (fluxo `mapear`, #208): as funcionalidades
 * por área, cada uma com os pontos de entrada, e os pontos sem funcionalidade
 * (os justificados e os que o app ainda acha sem tratamento). O analista aceita,
 * edita, junta, divide ou descarta cada funcionalidade; ao aprovar, as aceitas
 * entram no catálogo do projeto. Sem `revisao` (análise aprovada), só leitura.
 */
export function FuncionalidadesAoVivo({
  funcionalidades,
  justificados,
  escopo,
  versao,
  carregarCompletude,
  revisao,
}: {
  funcionalidades: Funcionalidade[];
  justificados: PontoJustificado[];
  escopo?: EscopoMapa | null;
  /** Versão da análise: muda a cada registro, para reler a completude. */
  versao?: number;
  carregarCompletude?: () => Promise<Completude>;
  revisao?: Revisao;
}) {
  const [completude, setCompletude] = useState<Completude | null>(null);
  useEffect(() => {
    let vivo = true;
    if (!carregarCompletude) return;
    Promise.resolve()
      .then(carregarCompletude)
      .then((c) => vivo && setCompletude(c ?? null))
      .catch(() => vivo && setCompletude(null));
    return () => {
      vivo = false;
    };
  }, [carregarCompletude, versao]);

  const abertas = funcionalidades.filter(aberta);
  const fechadas = funcionalidades.filter((f) => !aberta(f));
  const areas = new Map<string, Funcionalidade[]>();
  for (const f of [...abertas].sort(
    (a, b) => a.area_nome.localeCompare(b.area_nome) || a.id.localeCompare(b.id),
  )) {
    areas.set(f.area_nome, [...(areas.get(f.area_nome) ?? []), f]);
  }
  const semTratamento = completude?.semTratamento ?? [];
  return (
    <section aria-label="Funcionalidades" className="funcionalidades-ao-vivo">
      <h3>Funcionalidades</h3>
      <p className="muted">
        {descricaoDoEscopo(escopo)} · {contar(abertas.length, "funcionalidade", "funcionalidades")} em{" "}
        {contar(areas.size, "área", "áreas")}
        {completude &&
          ` · ${contar(completude.total, "ponto de entrada", "pontos de entrada")}, ${semTratamento.length} sem tratamento`}
      </p>
      {funcionalidades.length === 0 && (
        <p className="muted">Nenhuma funcionalidade registrada ainda.</p>
      )}
      {[...areas.entries()].map(([area, lista]) => (
        <div key={area} className="area-funcionalidades">
          <h4>{area}</h4>
          {lista.map((f) => (
            <CartaoFuncionalidade
              key={f.id}
              f={f}
              outras={abertas.filter((o) => o.id !== f.id)}
              revisao={revisao}
            />
          ))}
        </div>
      ))}
      {fechadas.length > 0 && (
        <details className="funcionalidades-fechadas">
          <summary>{contar(fechadas.length, "descartada ou juntada", "descartadas ou juntadas")}</summary>
          {fechadas.map((f) => (
            <CartaoFuncionalidade key={f.id} f={f} outras={[]} revisao={revisao} />
          ))}
        </details>
      )}
      <section aria-label="Pontos de entrada sem funcionalidade" className="pontos-sem-funcionalidade">
        <h4>Pontos de entrada sem funcionalidade</h4>
        {semTratamento.length > 0 && (
          <>
            <p className="aviso">
              {contar(semTratamento.length, "ponto de entrada ainda sem tratamento", "pontos de entrada ainda sem tratamento")}
              : o agente precisa atribuí-los ou justificá-los antes de concluir.
            </p>
            <ul aria-label="Sem tratamento">
              {semTratamento.map((p) => (
                <li key={p.chave}>
                  <code>{p.nome}</code>{" "}
                  <span className="muted">
                    · {NOME_TIPO_ENTRADA[p.tipo]} · {p.arquivo}:{p.linha}
                  </span>
                </li>
              ))}
            </ul>
          </>
        )}
        {justificados.length === 0 && semTratamento.length === 0 && (
          <p className="muted">Todos os pontos de entrada têm funcionalidade.</p>
        )}
        {justificados.length > 0 && (
          <ul aria-label="Justificados">
            {justificados.map((j) => (
              <li key={j.chave}>
                <code>{j.nome}</code>{" "}
                <span className="muted">
                  · {NOME_TIPO_ENTRADA[j.tipo]} · {NOME_MOTIVO[j.motivo]}:
                </span>{" "}
                {j.justificativa}
              </li>
            ))}
          </ul>
        )}
      </section>
    </section>
  );
}
