import { useState, type FormEvent } from "react";
import type {
  Criterio,
  Fundamento,
  Gherkin,
  Historia,
  LetraInvest,
  Narrativa,
  SituacaoHistoria,
} from "./analise";
import type { Revisao } from "./AcoesRevisao";
import type { DecisaoRevisao } from "./revisao";

/** As letras do INVEST, na ordem. */
const INVEST: [LetraInvest, string, string][] = [
  ["independente", "I", "independente"],
  ["negociavel", "N", "negociável"],
  ["valiosa", "V", "valiosa"],
  ["estimavel", "E", "estimável"],
  ["pequena", "S", "pequena"],
  ["testavel", "T", "testável"],
];

const SITUACAO: Record<SituacaoHistoria, string> = {
  pendente: "sem decisão",
  aceita: "aceita",
  descartada: "descartada",
  quebrada: "quebrada em fatias",
};

function validos(h: Historia): Criterio[] {
  return h.criterios.filter((c) => c.situacao !== "descartado");
}

/**
 * Critérios INVEST que a história não atende, com o motivo: os do agente e, como
 * no app (`Historia::alertas_invest`), o de testável quando nenhum critério válido
 * pode ser testado já.
 */
export function alertasInvest(h: Historia): Map<LetraInvest, string> {
  const alertas = new Map((h.invest ?? []).map((a) => [a.criterio, a.motivo] as const));
  if (!alertas.has("testavel")) {
    const v = validos(h);
    if (v.length === 0) alertas.set("testavel", "a história ainda não tem critério de aceite");
    else if (v.every((c) => c.pergunta))
      alertas.set("testavel", "todos os critérios dependem de pergunta em aberto");
  }
  return alertas;
}

function rotuloFundamento(f: Fundamento): string {
  return f.tipo === "afirmacao" ? `af. ${f.ref}` : f.id;
}

function textoFundamento(f: Fundamento): string {
  return f.tipo === "afirmacao" ? f.texto : f.titulo;
}

function SeloInvest({ historia }: { historia: Historia }) {
  const alertas = alertasInvest(historia);
  return (
    <span className="invest" aria-label="Verificação INVEST">
      {INVEST.map(([chave, letra, nome]) => {
        const motivo = alertas.get(chave);
        return (
          <span
            key={chave}
            className={motivo ? "invest-falha" : "invest-ok"}
            title={motivo ? `${nome}: ${motivo}` : `${nome}: atende`}
            aria-label={motivo ? `${nome}: não atende` : `${nome}: atende`}
          >
            {letra}
          </span>
        );
      })}
    </span>
  );
}

function FormNarrativa({
  historia,
  ocupado,
  aoSalvar,
  aoCancelar,
}: {
  historia: Historia;
  ocupado: boolean;
  aoSalvar: (n: Narrativa) => void;
  aoCancelar: () => void;
}) {
  const [n, setN] = useState<Narrativa>({
    titulo: historia.titulo,
    como: historia.como,
    quero: historia.quero,
    para: historia.para,
  });
  const campo = (chave: keyof Narrativa, rotulo: string) => (
    <label>
      {rotulo}
      <input value={n[chave]} onChange={(e) => setN({ ...n, [chave]: e.target.value })} />
    </label>
  );
  const enviar = (e: FormEvent) => {
    e.preventDefault();
    aoSalvar(n);
  };
  return (
    <form className="form-historia" aria-label={`Editar ${historia.id}`} onSubmit={enviar}>
      {campo("titulo", "Título")}
      {campo("como", "Como")}
      {campo("quero", "Quero")}
      {campo("para", "Para")}
      <div className="acoes">
        <button type="submit" className="btn btn-primary" disabled={ocupado}>
          Salvar
        </button>
        <button type="button" className="btn" onClick={aoCancelar}>
          Cancelar
        </button>
      </div>
    </form>
  );
}

function FormGherkin({
  criterio,
  ocupado,
  aoSalvar,
  aoCancelar,
}: {
  criterio: Criterio;
  ocupado: boolean;
  aoSalvar: (g: Gherkin) => void;
  aoCancelar: () => void;
}) {
  const [g, setG] = useState<Gherkin>({
    dado: criterio.dado,
    quando: criterio.quando,
    entao: criterio.entao,
  });
  const campo = (chave: keyof Gherkin, rotulo: string) => (
    <label>
      {rotulo}
      <textarea
        rows={2}
        value={g[chave]}
        onChange={(e) => setG({ ...g, [chave]: e.target.value })}
      />
    </label>
  );
  return (
    <form
      className="form-criterio"
      aria-label={`Editar CA-${criterio.numero}`}
      onSubmit={(e) => {
        e.preventDefault();
        aoSalvar(g);
      }}
    >
      {campo("dado", "Dado")}
      {campo("quando", "Quando")}
      {campo("entao", "Então")}
      <div className="acoes">
        <button type="submit" className="btn btn-primary" disabled={ocupado}>
          Salvar
        </button>
        <button type="button" className="btn" onClick={aoCancelar}>
          Cancelar
        </button>
      </div>
    </form>
  );
}

function ItemCriterio({
  historia,
  criterio,
  revisao,
  aberta,
}: {
  historia: Historia;
  criterio: Criterio;
  revisao?: Revisao;
  /** A história aceita ou pendente: os critérios podem ser decididos. */
  aberta: boolean;
}) {
  const [editando, setEditando] = useState(false);
  const c = criterio;
  const decidir = (d: DecisaoRevisao) =>
    revisao
      ?.aoDecidir(d)
      .then(() => setEditando(false))
      .catch(() => undefined);
  const ocupado = revisao?.ocupado ?? true;
  return (
    <li className={`criterio situacao-${c.situacao}`} aria-label={`CA-${c.numero}`}>
      <div className="criterio-cabecalho">
        <strong>CA-{c.numero}</strong>
        {c.pergunta && (
          <span className="badge" data-tone="attention">
            a confirmar: pergunta {c.pergunta}
          </span>
        )}
        {c.situacao !== "pendente" && <span className="muted">{c.situacao}</span>}
        {c.original && <span className="muted editada">(editado pelo analista)</span>}
      </div>
      {editando && revisao ? (
        <FormGherkin
          criterio={c}
          ocupado={ocupado}
          aoCancelar={() => setEditando(false)}
          aoSalvar={(g) =>
            decidir({
              tipo: "editar-criterio",
              historia: historia.id,
              numero: c.numero,
              ...g,
            })
          }
        />
      ) : (
        <dl className="gherkin">
          <dt>Dado</dt>
          <dd>{c.dado}</dd>
          <dt>Quando</dt>
          <dd>{c.quando}</dd>
          <dt>Então</dt>
          <dd>{c.entao}</dd>
        </dl>
      )}
      <p className="origens">
        Origem:{" "}
        {c.fundamentos.map((f) => (
          <span key={rotuloFundamento(f)} className="chip" title={textoFundamento(f)}>
            {rotuloFundamento(f)}
          </span>
        ))}
      </p>
      {revisao && aberta && !editando && (
        <div className="acoes-revisao" role="group" aria-label={`Revisão do CA-${c.numero}`}>
          {c.situacao !== "aceito" && (
            <button
              type="button"
              className="btn"
              disabled={ocupado}
              onClick={() =>
                decidir({
                  tipo: "criterio",
                  historia: historia.id,
                  numero: c.numero,
                  situacao: "aceito",
                })
              }
            >
              Aceitar
            </button>
          )}
          <button
            type="button"
            className="btn"
            disabled={ocupado}
            onClick={() => setEditando(true)}
          >
            Editar
          </button>
          {c.situacao !== "descartado" ? (
            <button
              type="button"
              className="btn"
              disabled={ocupado}
              onClick={() =>
                decidir({
                  tipo: "criterio",
                  historia: historia.id,
                  numero: c.numero,
                  situacao: "descartado",
                })
              }
            >
              Descartar
            </button>
          ) : (
            <button
              type="button"
              className="btn"
              disabled={ocupado}
              onClick={() =>
                decidir({
                  tipo: "criterio",
                  historia: historia.id,
                  numero: c.numero,
                  situacao: "pendente",
                })
              }
            >
              Desfazer
            </button>
          )}
        </div>
      )}
    </li>
  );
}

type Form = "editar" | "quebrar" | null;

function CartaoHistoria({ historia, revisao }: { historia: Historia; revisao?: Revisao }) {
  const [form, setForm] = useState<Form>(null);
  const [pedido, setPedido] = useState("");
  const h = historia;
  const aberta = h.situacao === "pendente" || h.situacao === "aceita";
  const ocupado = revisao?.ocupado ?? true;
  const decidir = (d: DecisaoRevisao) =>
    revisao
      ?.aoDecidir(d)
      .then(() => setForm(null))
      .catch(() => undefined);
  const alertas = alertasInvest(h);
  return (
    <section className={`historia situacao-${h.situacao}`} aria-label={`${h.id}: ${h.titulo}`}>
      <header className="historia-cabecalho">
        <span className="painel-id">{h.id}</span>
        <strong>{h.titulo}</strong>
        <SeloInvest historia={h} />
        <span className="badge" data-tone={h.situacao === "aceita" ? "done" : undefined}>
          {h.situacao === "pendente" && h.quebra_pedida != null
            ? "quebra pedida"
            : SITUACAO[h.situacao]}
        </span>
      </header>
      {form === "editar" && revisao ? (
        <FormNarrativa
          historia={h}
          ocupado={ocupado}
          aoCancelar={() => setForm(null)}
          aoSalvar={(n) => decidir({ tipo: "editar-historia", historia: h.id, ...n })}
        />
      ) : (
        <p className="narrativa">
          <strong>Como</strong> {h.como}, <strong>quero</strong> {h.quero}, <strong>para</strong>{" "}
          {h.para}
          {h.original && <span className="muted editada"> (editada pelo analista)</span>}
        </p>
      )}
      {h.fatia_de && <p className="muted">Fatia de {h.fatia_de}.</p>}
      {alertas.size > 0 && (
        <ul className="alertas-invest" aria-label={`Alertas INVEST de ${h.id}`}>
          {INVEST.filter(([chave]) => alertas.has(chave)).map(([chave, letra, nome]) => (
            <li key={chave}>
              <strong>{letra}</strong> ({nome}): {alertas.get(chave)}
            </li>
          ))}
        </ul>
      )}
      {h.quebra && (
        <p className="quebra">
          <strong>Quebra proposta:</strong> {h.quebra}
        </p>
      )}
      {h.situacao !== "quebrada" && (
        <ol className="criterios">
          {h.criterios.map((c) => (
            <ItemCriterio
              key={c.numero}
              historia={h}
              criterio={c}
              revisao={revisao}
              aberta={aberta}
            />
          ))}
        </ol>
      )}
      {form === "quebrar" && revisao && (
        <form
          className="form-quebra"
          aria-label={`Quebrar ${h.id}`}
          onSubmit={(e) => {
            e.preventDefault();
            decidir({
              tipo: "historia",
              historia: h.id,
              decisao: "quebrar",
              pedido: pedido.trim() || undefined,
            });
          }}
        >
          <label>
            Como quebrar (opcional)
            <textarea rows={2} value={pedido} onChange={(e) => setPedido(e.target.value)} />
          </label>
          <div className="acoes">
            <button type="submit" className="btn btn-primary" disabled={ocupado}>
              Pedir a quebra
            </button>
            <button type="button" className="btn" onClick={() => setForm(null)}>
              Cancelar
            </button>
          </div>
        </form>
      )}
      {revisao && h.situacao !== "quebrada" && form === null && (
        <div className="acoes-revisao" role="group" aria-label={`Revisão de ${h.id}`}>
          {h.situacao !== "aceita" && h.situacao !== "descartada" && (
            <button
              type="button"
              className="btn"
              disabled={ocupado}
              onClick={() =>
                decidir({
                  tipo: "historia",
                  historia: h.id,
                  decisao: "aceitar",
                })
              }
            >
              Aceitar
            </button>
          )}
          {aberta && (
            <>
              <button
                type="button"
                className="btn"
                disabled={ocupado}
                onClick={() => setForm("editar")}
              >
                Editar
              </button>
              <button
                type="button"
                className="btn"
                disabled={ocupado}
                onClick={() => {
                  setPedido(h.quebra_pedida ?? "");
                  setForm("quebrar");
                }}
              >
                Quebrar
              </button>
            </>
          )}
          {h.situacao !== "descartada" && (
            <button
              type="button"
              className="btn"
              disabled={ocupado}
              onClick={() =>
                decidir({
                  tipo: "historia",
                  historia: h.id,
                  decisao: "descartar",
                })
              }
            >
              Descartar
            </button>
          )}
          {(h.situacao !== "pendente" || h.quebra_pedida != null) && (
            <button
              type="button"
              className="btn"
              disabled={ocupado}
              onClick={() =>
                decidir({
                  tipo: "historia",
                  historia: h.id,
                  decisao: "desfazer",
                })
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
 * Histórias e critérios de aceite em revisão (fluxo `historias`, #115): cada
 * história com o "Como / quero / para", a verificação INVEST letra a letra e os
 * critérios Dado / Quando / Então com as afirmações e as regras de origem. O
 * analista aceita, edita, descarta ou pede a quebra de cada história e aceita,
 * edita ou descarta cada critério. Sem `revisao` (análise aprovada), só leitura.
 */
export function HistoriasAoVivo({
  historias,
  origem,
  revisao,
}: {
  historias: Historia[];
  origem?: string | null;
  revisao?: Revisao;
}) {
  const criterios = historias
    .filter((h) => h.situacao !== "descartada" && h.situacao !== "quebrada")
    .flatMap(validos);
  const aConfirmar = criterios.filter((c) => c.pergunta).length;
  return (
    <section aria-label="Histórias" className="historias-ao-vivo">
      <h3>Histórias</h3>
      <p className="muted">
        {historias.length} história(s) · {criterios.length} critério(s)
        {aConfirmar > 0 && ` · ${aConfirmar} a confirmar`}
        {origem && ` · origem ${origem}`}
      </p>
      <p className="muted legenda-invest">
        INVEST: I independente · N negociável · V valiosa · E estimável · S pequena · T testável
      </p>
      {historias.length === 0 && <p className="muted">Nenhuma história registrada ainda.</p>}
      {historias.map((h) => (
        <CartaoHistoria key={h.id} historia={h} revisao={revisao} />
      ))}
    </section>
  );
}
