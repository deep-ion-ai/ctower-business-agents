import type {
  Achado,
  Analise,
  AvaliacaoInvest,
  Caracteristica,
  Severidade,
  Veredito,
} from "./analise";
import type { DecisaoRevisao } from "./revisao";

/** Rótulos da revisão de requisito (#116), espelho de `cba_core::qualidade`. */
export const ROTULO_CARACTERISTICA: Record<Caracteristica, string> = {
  correcao: "Correção",
  ambiguidade: "Ambiguidade",
  verificabilidade: "Verificabilidade",
  completude: "Completude",
  consistencia: "Consistência",
  singularidade: "Singularidade",
};

const ROTULO_SEVERIDADE: Record<Severidade, string> = {
  alta: "alta",
  media: "média",
  baixa: "baixa",
};

const SEVERIDADES: Severidade[] = ["alta", "media", "baixa"];

export const ROTULO_VEREDITO: Record<Veredito, string> = {
  pronta: "Pronta",
  pronta_com_ressalvas: "Pronta com ressalvas",
  precisa_de_retrabalho: "Precisa de retrabalho",
};

const TOM_VEREDITO: Record<Veredito, string> = {
  pronta: "done",
  pronta_com_ressalvas: "attention",
  precisa_de_retrabalho: "error",
};

const ROTULO_INVEST: Record<AvaliacaoInvest["criterio"], string> = {
  independente: "Independente",
  negociavel: "Negociável",
  valiosa: "Valiosa",
  estimavel: "Estimável",
  pequena: "Pequena",
  testavel: "Testável",
};

const ROTULO_RESULTADO: Record<AvaliacaoInvest["resultado"], string> = {
  atende: "atende",
  atende_em_parte: "atende em parte",
  nao_atende: "não atende",
};

/** Contagem do resumo do veredito ("1 contradição", "3 ambiguidades"). */
const CONTAGEM: Record<Caracteristica, [string, string]> = {
  correcao: ["contradição", "contradições"],
  ambiguidade: ["ambiguidade", "ambiguidades"],
  verificabilidade: ["critério não verificável", "critérios não verificáveis"],
  completude: ["lacuna", "lacunas"],
  consistencia: ["termo fora do glossário", "termos fora do glossário"],
  singularidade: ["necessidade a separar", "necessidades a separar"],
};

export type Marca = "contradicao" | "ambiguidade";

/** Trecho do texto, marcado ou não. */
export interface Parte {
  texto: string;
  marca?: Marca;
}

/**
 * Divide o texto revisado nas partes marcadas pelos trechos dos achados ativos: a
 * correção marca como contradição; as demais características com trecho, como
 * ambiguidade. A busca ignora maiúsculas; trecho que não aparece (ou que se
 * sobrepõe a outro já marcado) fica sem marca.
 */
export function marcarTrechos(texto: string, achados: Achado[]): Parte[] {
  const minusculo = texto.toLowerCase();
  const marcas: { inicio: number; fim: number; marca: Marca }[] = [];
  // As contradições primeiro: numa sobreposição, ficam elas.
  const ordenados = [...achados]
    .filter((a) => a.estado === "registrado" && a.trecho.trim() !== "")
    .sort(
      (a, b) => Number(b.caracteristica === "correcao") - Number(a.caracteristica === "correcao"),
    );
  for (const a of ordenados) {
    const trecho = a.trecho.trim().toLowerCase();
    const inicio = minusculo.indexOf(trecho);
    if (inicio < 0) continue;
    const fim = inicio + trecho.length;
    if (marcas.some((m) => inicio < m.fim && fim > m.inicio)) continue;
    marcas.push({
      inicio,
      fim,
      marca: a.caracteristica === "correcao" ? "contradicao" : "ambiguidade",
    });
  }
  marcas.sort((a, b) => a.inicio - b.inicio);
  const partes: Parte[] = [];
  let pos = 0;
  for (const m of marcas) {
    if (m.inicio > pos) partes.push({ texto: texto.slice(pos, m.inicio) });
    partes.push({ texto: texto.slice(m.inicio, m.fim), marca: m.marca });
    pos = m.fim;
  }
  if (pos < texto.length) partes.push({ texto: texto.slice(pos) });
  return partes;
}

interface Props {
  analise: Analise;
  /** Descartar ou restaurar um achado; sem ela, ou com a análise aprovada, só leitura. */
  aoDecidir?: (d: DecisaoRevisao) => Promise<void>;
  ocupado?: boolean;
}

/**
 * Parecer de revisão de um requisito ou história (#116), ao vivo: o texto revisado
 * com as contradições e ambiguidades marcadas, o veredito e os achados por
 * severidade, cada um com a característica da ISO/IEC/IEEE 29148, o trecho, as
 * regras, as evidências e a reescrita sugerida, e a verificação INVEST.
 */
export function ParecerAoVivo({ analise, aoDecidir, ocupado }: Props) {
  const parecer = analise.parecer;
  const achados = parecer?.achados ?? [];
  const ativos = achados.filter((a) => a.estado === "registrado");
  const descartados = achados.filter((a) => a.estado === "descartado");
  const editavel = !!aoDecidir && analise.estado !== "aprovada";
  const veredito = parecer?.veredito ?? null;
  const partes = marcarTrechos(analise.entrada, achados);

  const contagem = (Object.keys(CONTAGEM) as Caracteristica[])
    .map((c) => [c, ativos.filter((a) => a.caracteristica === c).length] as const)
    .filter(([, n]) => n > 0);

  const decidir = (d: DecisaoRevisao) => {
    aoDecidir?.(d).catch(() => undefined);
  };

  return (
    <article className="documento-ao-vivo parecer-ao-vivo" aria-label="Parecer de revisão">
      <section className="secao" aria-label="Requisito revisado">
        <h3>Requisito revisado</h3>
        <p className="requisito-revisado">
          {partes.map((p, i) =>
            p.marca ? (
              <mark key={i} className={`marca-${p.marca}`}>
                {p.texto}
              </mark>
            ) : (
              <span key={i}>{p.texto}</span>
            ),
          )}
        </p>
        <p className="muted legenda-marcas">
          <mark className="marca-ambiguidade">ambiguidade</mark>{" "}
          <mark className="marca-contradicao">contradiz a base ou o código</mark>
        </p>
      </section>

      <section className="secao veredito" aria-label="Veredito">
        <h3>Veredito</h3>
        {veredito ? (
          <p>
            <span className="badge" data-tone={TOM_VEREDITO[veredito.veredito]}>
              {ROTULO_VEREDITO[veredito.veredito]}
            </span>{" "}
            {veredito.justificativa}
          </p>
        ) : (
          <p className="muted">Veredito ainda não registrado.</p>
        )}
        {contagem.length > 0 && (
          <p className="contagem-achados">
            {contagem.map(([c, n]) => (
              <span key={c} className="badge">
                {n} {CONTAGEM[c][n === 1 ? 0 : 1]}
              </span>
            ))}
          </p>
        )}
      </section>

      <section className="secao" aria-label="Achados">
        <h3>Achados</h3>
        {ativos.length === 0 && <p className="muted">Nenhum achado registrado.</p>}
        {SEVERIDADES.map((s) => {
          const daqui = ativos.filter((a) => a.severidade === s);
          if (daqui.length === 0) return null;
          return (
            <ol key={s} className="achados" aria-label={`Severidade ${ROTULO_SEVERIDADE[s]}`}>
              {daqui.map((a) => (
                <li key={a.numero} className="achado" data-caracteristica={a.caracteristica}>
                  <div className="achado-cabecalho">
                    <strong>{ROTULO_CARACTERISTICA[a.caracteristica]}</strong>
                    <span
                      className="badge"
                      data-tone={s === "alta" ? "error" : s === "media" ? "attention" : undefined}
                    >
                      {ROTULO_SEVERIDADE[s]}
                    </span>
                    <span className="muted">achado {a.numero}</span>
                    {editavel && (
                      <button
                        type="button"
                        className="btn btn-sm"
                        disabled={ocupado}
                        onClick={() =>
                          decidir({
                            tipo: "achado",
                            numero: a.numero,
                            estado: "descartado",
                          })
                        }
                      >
                        Descartar
                      </button>
                    )}
                  </div>
                  {a.trecho && <p className="achado-trecho">“{a.trecho}”</p>}
                  <p>{a.problema}</p>
                  {a.regras && a.regras.length > 0 && (
                    <p className="muted">
                      Regras da base: <span className="mono">{a.regras.join(", ")}</span>
                    </p>
                  )}
                  {a.evidencias.length > 0 && (
                    <ul className="evidencias" aria-label="Evidências">
                      {a.evidencias.map((e) => (
                        <li key={`${e.arquivo}:${e.linhas}`} className="mono">
                          {e.arquivo}:{e.linhas}
                          {e.estado === "a-revalidar" && (
                            <span className="badge" data-tone="attention">
                              a revalidar
                            </span>
                          )}
                        </li>
                      ))}
                    </ul>
                  )}
                  {a.reescrita && (
                    <p className="achado-reescrita">
                      <span className="muted">Reescrita sugerida:</span> {a.reescrita}
                    </p>
                  )}
                </li>
              ))}
            </ol>
          );
        })}
        {descartados.length > 0 && (
          <details className="achados-descartados">
            <summary>Descartados ({descartados.length})</summary>
            <ul>
              {descartados.map((a) => (
                <li key={a.numero}>
                  <span className="muted">
                    {ROTULO_CARACTERISTICA[a.caracteristica]} · achado {a.numero}:
                  </span>{" "}
                  {a.problema}
                  {editavel && (
                    <button
                      type="button"
                      className="btn btn-sm"
                      disabled={ocupado}
                      onClick={() =>
                        decidir({
                          tipo: "achado",
                          numero: a.numero,
                          estado: "registrado",
                        })
                      }
                    >
                      Restaurar
                    </button>
                  )}
                </li>
              ))}
            </ul>
          </details>
        )}
      </section>

      {parecer?.invest && parecer.invest.length > 0 && (
        <section className="secao" aria-label="Verificação INVEST">
          <h3>Verificação INVEST</h3>
          <table className="tabela-invest">
            <thead>
              <tr>
                <th>Critério</th>
                <th>Resultado</th>
                <th>Justificativa</th>
              </tr>
            </thead>
            <tbody>
              {parecer.invest.map((i) => (
                <tr key={i.criterio} data-resultado={i.resultado}>
                  <td>{ROTULO_INVEST[i.criterio]}</td>
                  <td>{ROTULO_RESULTADO[i.resultado]}</td>
                  <td>{i.justificativa}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </section>
      )}
    </article>
  );
}
