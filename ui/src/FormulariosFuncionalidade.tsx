import { useState } from "react";
import type { EstadoAnalise } from "./analise";
import type { Funcionalidade, RevisaoDoMapa, SituacaoFuncionalidade } from "./funcionalidades";

/** Revisão do mapa (#208): formulários e rótulos da tela do mapa e do painel. */

export const SITUACAO: Record<SituacaoFuncionalidade, string> = {
  pendente: "a revisar",
  aceita: "aceita",
  descartada: "descartada",
  juntada: "juntada",
};

export const ESTADO_ANALISE: Record<EstadoAnalise, string> = {
  criada: "criada",
  "em-andamento": "em andamento",
  concluida: "concluída",
  aprovada: "aprovada",
};

export interface Revisao {
  ocupado: boolean;
  aoRevisar: (r: RevisaoDoMapa) => Promise<void>;
}

export type Form = "editar" | "juntar" | "dividir" | null;

export function FormEditar({
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
  const [area, setArea] = useState(f.areaNome);
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

export function FormJuntar({
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
        Os pontos de entrada, os atores e os dados de {f.id} passam para a escolhida, e {f.id} sai
        do mapa (o identificador não é reaproveitado).
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

export function FormDividir({
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

