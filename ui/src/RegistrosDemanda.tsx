import { useCallback, useEffect, useState, type FormEvent } from "react";
import { mensagemDe } from "./analise";
import {
  CATEGORIAS,
  NOME_CATEGORIA,
  ROTULO_NIVEL,
  ROTULO_RESTRICAO,
  ROTULO_STATUS_PREMISSA,
  TITULO_CATEGORIA,
  apiRegistrosTauri,
  rotuloReferencia,
  type ApiRegistros,
  type Categoria,
  type DadosRegistro,
  type Nivel,
  type PessoaVista,
  type Proposta,
  type RegistroVista,
  type RegistrosDaDemanda,
  type StatusPremissa,
  type TipoRestricao,
} from "./registros";
import "./registros.css";

const NIVEIS: Nivel[] = ["baixo", "medio", "alto"];
const RESTRICOES: TipoRestricao[] = ["prazo", "legal", "tecnica"];

const QUEM: Record<Categoria, string> = {
  premissa: "Quem assumiu",
  decisao: "Quem decidiu",
  risco: "Responsável",
  restricao: "Quem informou",
};

const TOM_STATUS: Record<StatusPremissa, string> = {
  "nao-validada": "attention",
  validada: "done",
  invalidada: "error",
};

function riscoAlto(r: { probabilidade?: Nivel | null; impacto?: Nivel | null }) {
  return r.probabilidade === "alto" || r.impacto === "alto";
}

/** Formulário aberto: criar, editar, ajustar uma proposta ou validar uma premissa. */
type Edicao =
  | { tipo: "nova"; categoria: Categoria }
  | { tipo: "editar"; registro: RegistroVista }
  | { tipo: "proposta"; proposta: Proposta }
  | { tipo: "validar"; registro: RegistroVista; status: StatusPremissa };

function dadosDoRegistro(r: RegistroVista): DadosRegistro {
  return {
    titulo: r.titulo,
    detalhe: r.detalhe,
    quem: r.quem,
    pessoa: r.pessoa,
    status: r.status,
    comoValidada: r.validacao?.como ?? null,
    decididoEm: r.decididoEm,
    alternativas: r.alternativas,
    probabilidade: r.probabilidade,
    impacto: r.impacto,
    resposta: r.resposta,
    restricao: r.restricao,
  };
}

function vazio(categoria: Categoria): DadosRegistro {
  return {
    titulo: "",
    status: categoria === "premissa" ? "nao-validada" : null,
    probabilidade: categoria === "risco" ? "medio" : null,
    impacto: categoria === "risco" ? "medio" : null,
    restricao: categoria === "restricao" ? "prazo" : null,
    alternativas: [],
  };
}

/**
 * Premissas, decisões, riscos e restrições da demanda (#122), no painel da demanda:
 * a faixa das propostas do agente (das análises de impacto e das atas) para o
 * analista aceitar, ajustar ou recusar; as premissas com status, quem assumiu e a
 * origem, as não validadas em destaque e os botões de validar; as decisões com quem
 * decidiu, a data, o item da ata e as alternativas; os riscos numa matriz
 * probabilidade × impacto, os altos em destaque; e as restrições por tipo. Os
 * registros ficam na pasta da demanda, em Markdown com frontmatter.
 */
export function RegistrosDemanda({
  projeto,
  demanda,
  api = apiRegistrosTauri,
  atualizacao,
  abrirDocumento,
  podeEditar = true,
}: {
  projeto: string;
  demanda: string;
  api?: ApiRegistros;
  /** Muda quando o painel relê a base: a aba relê junto. */
  atualizacao?: unknown;
  abrirDocumento?: (caminho: string) => void;
  podeEditar?: boolean;
}) {
  const [aba, setAba] = useState<RegistrosDaDemanda | null>(null);
  const [falha, setFalha] = useState<string | null>(null);
  const [edicao, setEdicao] = useState<Edicao | null>(null);
  const [ocupado, setOcupado] = useState(false);

  const carregar = useCallback(() => {
    api
      .registros(projeto, demanda)
      .then((r) => {
        if (r && Array.isArray(r.registros)) {
          setAba(r);
          setFalha(null);
        }
      })
      .catch((e: unknown) => setFalha(mensagemDe(e)));
  }, [api, projeto, demanda]);

  useEffect(carregar, [carregar, atualizacao]);

  const executar = (acao: () => Promise<unknown>) => {
    setOcupado(true);
    setFalha(null);
    acao()
      .then(() => {
        setEdicao(null);
        carregar();
      })
      .catch((e: unknown) => setFalha(mensagemDe(e)))
      .finally(() => setOcupado(false));
  };

  const aceitar = (p: Proposta, ajustes?: DadosRegistro) =>
    executar(() => api.aceitarProposta(projeto, demanda, p.referencia, ajustes));
  const recusar = (p: Proposta) =>
    executar(() => api.recusarProposta(projeto, demanda, p.referencia));

  if (!aba) {
    return (
      <section aria-label="Premissas, decisões, riscos e restrições" className="registros-demanda">
        <h3>Premissas, decisões, riscos e restrições</h3>
        {falha ? (
          <p className="error-text" role="alert">
            {falha}
          </p>
        ) : (
          <p className="muted">Carregando…</p>
        )}
      </section>
    );
  }

  const daCategoria = (c: Categoria) => aba.registros.filter((r) => r.categoria === c);
  const propostasDa = (c: Categoria) => aba.propostas.filter((p) => p.categoria === c);
  const naoValidadas = daCategoria("premissa").filter((r) => r.status === "nao-validada").length;
  const altos = daCategoria("risco").filter(riscoAlto).length;

  const salvar = (dados: DadosRegistro) => {
    if (!edicao) return;
    switch (edicao.tipo) {
      case "nova":
        return executar(() => api.criar(projeto, demanda, edicao.categoria, dados));
      case "editar":
        return executar(() => api.editar(projeto, edicao.registro.id, dados));
      case "proposta":
        return aceitar(edicao.proposta, dados);
    }
  };

  const contador = (c: Categoria) => {
    if (c === "premissa" && naoValidadas > 0)
      return (
        <span className="badge" data-tone="attention">
          {naoValidadas} não validada{naoValidadas > 1 ? "s" : ""}
        </span>
      );
    if (c === "risco" && altos > 0)
      return (
        <span className="badge" data-tone="error">
          {altos} alto{altos > 1 ? "s" : ""}
        </span>
      );
    return <span className="badge">{daCategoria(c).length}</span>;
  };

  return (
    <section aria-label="Premissas, decisões, riscos e restrições" className="registros-demanda">
      <header className="registros-cabecalho">
        <h3>Premissas, decisões, riscos e restrições</h3>
        {podeEditar && (
          <div className="registros-novos">
            {CATEGORIAS.map((c) => (
              <button
                key={c}
                type="button"
                className="btn"
                onClick={() => setEdicao({ tipo: "nova", categoria: c })}
              >
                {c === "premissa" || c === "decisao" || c === "restricao" ? "Nova" : "Novo"}{" "}
                {NOME_CATEGORIA[c]}
              </button>
            ))}
          </div>
        )}
      </header>
      {falha && (
        <p className="error-text" role="alert">
          {falha}
        </p>
      )}

      {aba.propostas.length > 0 && <FaixaPropostas propostas={aba.propostas} />}

      {edicao && edicao.tipo !== "validar" && (
        <FormRegistro
          key={
            edicao.tipo === "nova"
              ? `nova:${edicao.categoria}`
              : edicao.tipo === "editar"
                ? edicao.registro.id
                : edicao.proposta.referencia
          }
          categoria={
            edicao.tipo === "nova"
              ? edicao.categoria
              : edicao.tipo === "editar"
                ? edicao.registro.categoria
                : edicao.proposta.categoria
          }
          titulo={
            edicao.tipo === "nova"
              ? `${edicao.categoria === "risco" ? "Novo" : "Nova"} ${NOME_CATEGORIA[edicao.categoria]}`
              : edicao.tipo === "editar"
                ? `Editar ${edicao.registro.id}`
                : `Ajustar a proposta ${edicao.proposta.referencia}`
          }
          inicial={
            edicao.tipo === "nova"
              ? vazio(edicao.categoria)
              : edicao.tipo === "editar"
                ? dadosDoRegistro(edicao.registro)
                : edicao.proposta.dados
          }
          botao={edicao.tipo === "proposta" ? "Aceitar" : "Salvar"}
          pessoas={aba.pessoas}
          ocupado={ocupado}
          aoSalvar={salvar}
          aoCancelar={() => setEdicao(null)}
        />
      )}
      {edicao?.tipo === "validar" && (
        <FormValidacao
          registro={edicao.registro}
          status={edicao.status}
          ocupado={ocupado}
          aoCancelar={() => setEdicao(null)}
          aoSalvar={(como) =>
            executar(() =>
              api.validarPremissa(projeto, edicao.registro.id, {
                status: edicao.status,
                como,
              }),
            )
          }
        />
      )}

      <div className="registros-grade">
        {CATEGORIAS.map((c) => (
          <section key={c} aria-label={TITULO_CATEGORIA[c]} className="registros-cartao">
            <h4>
              {TITULO_CATEGORIA[c]} {contador(c)}
            </h4>
            {c === "risco" && daCategoria(c).length > 0 && <MatrizRiscos riscos={daCategoria(c)} />}
            <ul className="registros-lista">
              {daCategoria(c).map((r) => (
                <LinhaRegistro
                  key={r.id}
                  registro={r}
                  podeEditar={podeEditar}
                  ocupado={ocupado}
                  abrirDocumento={abrirDocumento}
                  editar={() => setEdicao({ tipo: "editar", registro: r })}
                  validar={(status) => {
                    if (status === "nao-validada") {
                      executar(() => api.validarPremissa(projeto, r.id, { status }));
                    } else {
                      setEdicao({ tipo: "validar", registro: r, status });
                    }
                  }}
                />
              ))}
              {propostasDa(c).map((p) => (
                <li key={p.referencia} className="registro-proposta" data-proposta>
                  <span>{p.dados.titulo}</span>
                  <div className="muted registro-meta">
                    <span className="badge" data-tone="running">
                      proposta do agente
                    </span>{" "}
                    {rotuloReferencia(p.referencia)}
                    {p.dados.quem && ` · ${p.dados.quem}`}
                    {p.dados.status && ` · sugere ${ROTULO_STATUS_PREMISSA[p.dados.status]}`}
                  </div>
                  {podeEditar && (
                    <div className="registro-acoes">
                      <button
                        type="button"
                        className="btn"
                        disabled={ocupado}
                        onClick={() => aceitar(p)}
                        aria-label={`Aceitar ${p.referencia}`}
                      >
                        Aceitar
                      </button>
                      <button
                        type="button"
                        className="btn"
                        disabled={ocupado}
                        onClick={() => setEdicao({ tipo: "proposta", proposta: p })}
                        aria-label={`Ajustar ${p.referencia}`}
                      >
                        Ajustar
                      </button>
                      <button
                        type="button"
                        className="btn"
                        disabled={ocupado}
                        onClick={() => recusar(p)}
                        aria-label={`Recusar ${p.referencia}`}
                      >
                        Recusar
                      </button>
                    </div>
                  )}
                </li>
              ))}
              {daCategoria(c).length === 0 && propostasDa(c).length === 0 && (
                <li className="muted">Nenhum registro.</li>
              )}
            </ul>
          </section>
        ))}
      </div>
      <p className="muted registros-pasta">
        Registros em <code>{aba.pasta}/</code>
      </p>
    </section>
  );
}

function FaixaPropostas({ propostas }: { propostas: Proposta[] }) {
  const fontes = new Map<string, { rotulo: string; contagem: Map<Categoria, number> }>();
  for (const p of propostas) {
    const chave = `${p.fonte.tipo}:${p.fonte.id}`;
    const rotulo =
      p.fonte.tipo === "analise"
        ? `${p.fonte.id} (impacto)`
        : `ata ${p.fonte.id} (reunião de ${p.fonte.data})`;
    const f = fontes.get(chave) ?? { rotulo, contagem: new Map() };
    f.contagem.set(p.categoria, (f.contagem.get(p.categoria) ?? 0) + 1);
    fontes.set(chave, f);
  }
  const partes = [...fontes.values()].map(({ rotulo, contagem }) => {
    const itens = [...contagem.entries()].map(
      ([c, n]) => `${n} ${n > 1 ? TITULO_CATEGORIA[c].toLowerCase() : NOME_CATEGORIA[c]}`,
    );
    return `De ${rotulo}: ${itens.join(", ")}.`;
  });
  return (
    <p className="registros-faixa" role="status">
      <span className="badge" data-tone="running">
        {propostas.length} proposta{propostas.length > 1 ? "s" : ""} do agente
      </span>{" "}
      {partes.join(" ")}
    </p>
  );
}

function LinhaRegistro({
  registro: r,
  podeEditar,
  ocupado,
  abrirDocumento,
  editar,
  validar,
}: {
  registro: RegistroVista;
  podeEditar: boolean;
  ocupado: boolean;
  abrirDocumento?: (caminho: string) => void;
  editar: () => void;
  validar: (status: StatusPremissa) => void;
}) {
  const destaque =
    (r.categoria === "premissa" && r.status === "nao-validada") ||
    (r.categoria === "risco" && riscoAlto(r));
  const origem = rotuloReferencia(r.referencia);
  return (
    <li data-destaque={destaque ? "sim" : undefined} data-registro={r.id}>
      <span className="mono registro-id">{r.id}</span>{" "}
      {r.categoria === "restricao" && r.restricao && (
        <span className="badge">{ROTULO_RESTRICAO[r.restricao]}</span>
      )}{" "}
      <span>{r.titulo}</span>
      <div className="muted registro-meta">
        {r.categoria === "premissa" && r.status && (
          <span className="badge" data-tone={TOM_STATUS[r.status]}>
            {ROTULO_STATUS_PREMISSA[r.status]}
          </span>
        )}
        {r.categoria === "premissa" && r.quem && ` assumida por ${r.quem}`}
        {r.categoria === "decisao" && r.quem && <> decidiu <strong>{r.quem}</strong></>}
        {r.categoria === "decisao" && r.decididoEm && ` · ${r.decididoEm}`}
        {r.categoria === "risco" &&
          r.probabilidade &&
          r.impacto &&
          ` ${ROTULO_NIVEL[r.probabilidade]} × ${ROTULO_NIVEL[r.impacto]}`}
        {r.categoria === "risco" && r.quem && ` · resp. ${r.quem}`}
        {r.categoria === "risco" && r.resposta && ` · ${r.resposta}`}
        {r.categoria === "restricao" && r.quem && ` informada por ${r.quem}`}
        {origem && ` · ${origem}`}
        {r.categoria === "decisao" && r.alternativas.length > 0 &&
          ` · alternativas: ${r.alternativas.join(", ")}`}
        {r.validacao && ` · ${r.validacao.como} (${r.validacao.em})`}
      </div>
      <div className="registro-acoes">
        {podeEditar && r.categoria === "premissa" && r.status !== "validada" && (
          <button type="button" className="btn" disabled={ocupado} onClick={() => validar("validada")}>
            Validar
          </button>
        )}
        {podeEditar && r.categoria === "premissa" && r.status !== "invalidada" && (
          <button
            type="button"
            className="btn"
            disabled={ocupado}
            onClick={() => validar("invalidada")}
          >
            Invalidar
          </button>
        )}
        {podeEditar && (
          <button type="button" className="btn" disabled={ocupado} onClick={editar}>
            Editar
          </button>
        )}
        {abrirDocumento && (
          <button type="button" className="link" onClick={() => abrirDocumento(r.documento)}>
            abrir
          </button>
        )}
        {abrirDocumento && r.origem && (
          <button type="button" className="link" onClick={() => abrirDocumento(r.origem!)}>
            abrir a origem
          </button>
        )}
      </div>
    </li>
  );
}

/** Matriz probabilidade (linhas, alta → baixa) × impacto (colunas, baixo → alto). */
function MatrizRiscos({ riscos }: { riscos: RegistroVista[] }) {
  const linhas: Nivel[] = ["alto", "medio", "baixo"];
  return (
    <table className="matriz-riscos" aria-label="Matriz probabilidade × impacto">
      <thead>
        <tr>
          <th scope="col">probabilidade \ impacto</th>
          {NIVEIS.map((i) => (
            <th key={i} scope="col">
              {ROTULO_NIVEL[i]}
            </th>
          ))}
        </tr>
      </thead>
      <tbody>
        {linhas.map((p) => (
          <tr key={p}>
            <th scope="row">{ROTULO_NIVEL[p]}</th>
            {NIVEIS.map((i) => {
              const aqui = riscos.filter((r) => r.probabilidade === p && r.impacto === i);
              return (
                <td key={i} data-nivel={p === "alto" || i === "alto" ? "alto" : undefined}>
                  {aqui.length === 0 ? "–" : aqui.map((r) => r.id).join(" ")}
                </td>
              );
            })}
          </tr>
        ))}
      </tbody>
    </table>
  );
}

function FormRegistro({
  categoria,
  titulo,
  inicial,
  botao,
  pessoas,
  ocupado,
  aoSalvar,
  aoCancelar,
}: {
  categoria: Categoria;
  titulo: string;
  inicial: DadosRegistro;
  botao: string;
  pessoas: PessoaVista[];
  ocupado: boolean;
  aoSalvar: (d: DadosRegistro) => void;
  aoCancelar: () => void;
}) {
  const [d, setD] = useState<DadosRegistro>(inicial);
  const [alternativas, setAlternativas] = useState((inicial.alternativas ?? []).join("\n"));
  const mudar = (campo: Partial<DadosRegistro>) => setD((atual) => ({ ...atual, ...campo }));
  const enviar = (e: FormEvent) => {
    e.preventDefault();
    aoSalvar({
      ...d,
      alternativas: alternativas
        .split("\n")
        .map((a) => a.trim())
        .filter(Boolean),
    });
  };
  const rotuloTitulo: Record<Categoria, string> = {
    premissa: "Enunciado da premissa",
    decisao: "O que foi decidido",
    risco: "Descrição do risco",
    restricao: "Restrição",
  };
  return (
    <form className="form-registro" aria-label={titulo} onSubmit={enviar}>
      <h4>{titulo}</h4>
      <label>
        {rotuloTitulo[categoria]}
        <input
          value={d.titulo}
          onChange={(e) => mudar({ titulo: e.target.value })}
          required
          maxLength={300}
        />
      </label>
      <label>
        {QUEM[categoria]} (catálogo de pessoas)
        <select
          value={d.pessoa ?? ""}
          onChange={(e) => {
            const slug = e.target.value || null;
            const pessoa = pessoas.find((p) => p.slug === slug);
            mudar({ pessoa: slug, quem: pessoa ? pessoa.nome : d.quem });
          }}
        >
          <option value="">fora do catálogo</option>
          {pessoas.map((p) => (
            <option key={p.slug} value={p.slug}>
              {p.nome}
              {p.papel ? ` (${p.papel})` : ""}
            </option>
          ))}
        </select>
      </label>
      <label>
        {QUEM[categoria]} (nome)
        <input value={d.quem ?? ""} onChange={(e) => mudar({ quem: e.target.value })} />
      </label>
      {categoria === "premissa" && (
        <>
          <label>
            Status
            <select
              value={d.status ?? "nao-validada"}
              onChange={(e) => mudar({ status: e.target.value as StatusPremissa })}
            >
              {(Object.keys(ROTULO_STATUS_PREMISSA) as StatusPremissa[]).map((s) => (
                <option key={s} value={s}>
                  {ROTULO_STATUS_PREMISSA[s]}
                </option>
              ))}
            </select>
          </label>
          {d.status && d.status !== "nao-validada" && (
            <label>
              Como foi validada
              <input
                value={d.comoValidada ?? ""}
                onChange={(e) => mudar({ comoValidada: e.target.value })}
              />
            </label>
          )}
        </>
      )}
      {categoria === "decisao" && (
        <>
          <label>
            Quando
            <input
              type="date"
              value={d.decididoEm ?? ""}
              onChange={(e) => mudar({ decididoEm: e.target.value || null })}
            />
          </label>
          <label>
            Alternativas consideradas (uma por linha)
            <textarea value={alternativas} onChange={(e) => setAlternativas(e.target.value)} />
          </label>
        </>
      )}
      {categoria === "risco" && (
        <>
          <label>
            Probabilidade
            <select
              value={d.probabilidade ?? "medio"}
              onChange={(e) => mudar({ probabilidade: e.target.value as Nivel })}
            >
              {NIVEIS.map((n) => (
                <option key={n} value={n}>
                  {ROTULO_NIVEL[n]}
                </option>
              ))}
            </select>
          </label>
          <label>
            Impacto
            <select
              value={d.impacto ?? "medio"}
              onChange={(e) => mudar({ impacto: e.target.value as Nivel })}
            >
              {NIVEIS.map((n) => (
                <option key={n} value={n}>
                  {ROTULO_NIVEL[n]}
                </option>
              ))}
            </select>
          </label>
          <label>
            Resposta
            <input value={d.resposta ?? ""} onChange={(e) => mudar({ resposta: e.target.value })} />
          </label>
        </>
      )}
      {categoria === "restricao" && (
        <label>
          Tipo
          <select
            value={d.restricao ?? "prazo"}
            onChange={(e) => mudar({ restricao: e.target.value as TipoRestricao })}
          >
            {RESTRICOES.map((t) => (
              <option key={t} value={t}>
                {ROTULO_RESTRICAO[t]}
              </option>
            ))}
          </select>
        </label>
      )}
      <label>
        Detalhe
        <textarea value={d.detalhe ?? ""} onChange={(e) => mudar({ detalhe: e.target.value })} />
      </label>
      <div className="registro-acoes">
        <button type="submit" className="btn" disabled={ocupado || !d.titulo.trim()}>
          {botao}
        </button>
        <button type="button" className="btn" onClick={aoCancelar}>
          Cancelar
        </button>
      </div>
    </form>
  );
}

function FormValidacao({
  registro,
  status,
  ocupado,
  aoSalvar,
  aoCancelar,
}: {
  registro: RegistroVista;
  status: StatusPremissa;
  ocupado: boolean;
  aoSalvar: (como: string) => void;
  aoCancelar: () => void;
}) {
  const [como, setComo] = useState("");
  const verbo = status === "validada" ? "Validar" : "Invalidar";
  return (
    <form
      className="form-registro"
      aria-label={`${verbo} ${registro.id}`}
      onSubmit={(e) => {
        e.preventDefault();
        aoSalvar(como);
      }}
    >
      <h4>
        {verbo} {registro.id}: {registro.titulo}
      </h4>
      <label>
        Como foi {status === "validada" ? "validada" : "invalidada"}
        <input
          value={como}
          onChange={(e) => setComo(e.target.value)}
          required
          placeholder="Confirmado pelo PO na reunião R-0002"
        />
      </label>
      <div className="registro-acoes">
        <button type="submit" className="btn" disabled={ocupado || !como.trim()}>
          {verbo}
        </button>
        <button type="button" className="btn" onClick={aoCancelar}>
          Cancelar
        </button>
      </div>
    </form>
  );
}
