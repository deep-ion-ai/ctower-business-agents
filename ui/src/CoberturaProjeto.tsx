import { useEffect, useState } from "react";
import { mensagemDe } from "./analise";
import { idadeEmTexto, type ApiCobertura, type Cobertura } from "./cobertura";
import { AjudaTermo } from "./AjudaTermo";
import { contar } from "./formatacao";
import type { Tela } from "./Navegacao";

const PASTAS_VISIVEIS = 8;

/** Cor do mapa de calor: só reforça; os números e o texto "lacuna" dizem o mesmo. */
function fundo(calor: number): string {
  return calor > 0
    ? `color-mix(in srgb, var(--attention, #f2a33a) ${Math.round(calor * 60) + 10}%, transparent)`
    : "transparent";
}

/**
 * Cobertura da base (#125), na tela do projeto: o que a base já documenta de cada
 * área e de cada pasta, cruzado com o quanto o código é referenciado, e a próxima
 * análise de maior valor. Cálculo sem IA, feito a cada abertura e a cada mudança
 * da base.
 */
export function CoberturaProjeto({
  api,
  projeto,
  navegar,
  versao,
}: {
  api: ApiCobertura;
  projeto: string;
  navegar: (t: Tela) => void;
  /** Muda quando a base muda (relê o cálculo). */
  versao?: unknown;
}) {
  const [cobertura, setCobertura] = useState<Cobertura | null>(null);
  const [falha, setFalha] = useState<string | null>(null);
  const [todasAsPastas, setTodasAsPastas] = useState(false);

  useEffect(() => {
    let ativo = true;
    api
      .cobertura(projeto)
      .then((c) => {
        if (!ativo) return;
        setCobertura(c);
        setFalha(null);
      })
      .catch((e: unknown) => ativo && setFalha(mensagemDe(e)));
    return () => {
      ativo = false;
    };
  }, [api, projeto, versao]);

  const sugestao = cobertura?.sugestao ?? null;
  const comecar = () => {
    if (!sugestao) return;
    if (sugestao.funcionalidade) navegar({ tipo: "mapa", projeto });
    else navegar({ tipo: "analise", projeto, pergunta: sugestao.entrada });
  };
  const pastas = cobertura?.pastas.filter((p) => p.centralidade > 0 || p.regras > 0) ?? [];
  const pastasMostradas = todasAsPastas ? pastas : pastas.slice(0, PASTAS_VISIVEIS);

  return (
    <section aria-label="Cobertura da base" className="cobertura-projeto">
      <h3>
        Cobertura da base
        <AjudaTermo termo="cobertura">
          Quanto do código do projeto já tem regras, termos e documentos na base.
        </AjudaTermo>
      </h3>
      {falha && (
        <p className="error-text" role="alert">
          {falha}
        </p>
      )}
      {!cobertura && !falha && <p className="muted">Calculando…</p>}
      {cobertura && (
        <>
          <p className="muted">
            {contar(cobertura.totais.regras, "regra", "regras")} ·{" "}
            {contar(cobertura.totais.termos, "termo", "termos")} ·{" "}
            {contar(cobertura.totais.documentos, "documento negocial", "documentos negociais")}
            {cobertura.totais.aRevalidar > 0 && ` · ${cobertura.totais.aRevalidar} a revalidar`}
          </p>

          {sugestao ? (
            <div className="sugestao-cobertura" role="region" aria-label="Próxima análise sugerida">
              <strong>Próxima análise sugerida: {sugestao.entrada}</strong>
              <p className="muted">{sugestao.motivo}</p>
              <button type="button" className="btn" onClick={comecar}>
                {sugestao.funcionalidade ? "Abrir o mapa de funcionalidades" : "Perguntar ao sistema"}
              </button>
            </div>
          ) : (
            <p className="muted">Nenhuma lacuna evidente: o código referenciado tem documentação.</p>
          )}

          {cobertura.areas.length > 0 && (
            <>
              <h4>Por área</h4>
              <ul className="mapa-calor" aria-label="Mapa de calor por área">
                {cobertura.areas.map((a) => (
                  <li
                    key={a.id}
                    className="celula-calor"
                    data-lacuna={a.lacuna || undefined}
                    style={{ background: fundo(a.calor) }}
                  >
                    <strong>{a.nome}</strong>
                    {a.lacuna && <span className="badge" data-tone="attention">lacuna</span>}
                    <div>
                      {contar(a.regras, "regra", "regras")} · {contar(a.termos, "termo", "termos")} ·{" "}
                      {contar(a.documentos, "documento", "documentos")}
                    </div>
                    <div className="muted">
                      referenciada {contar(a.centralidade, "vez", "vezes")}
                      {a.funcionalidades > 0 &&
                        ` · ${a.funcionalidades - a.funcionalidadesSemDocumento} de ${a.funcionalidades} funcionalidades documentadas`}
                    </div>
                    <div className="muted">
                      atualizada há {idadeEmTexto(a.idadeRecenteDias)}
                      {a.aRevalidar > 0 && ` · ${a.aRevalidar} a revalidar`}
                      {a.semConfirmacaoRecente > 0 &&
                        ` · ${contar(a.semConfirmacaoRecente, "regra", "regras")} sem confirmação recente`}
                    </div>
                  </li>
                ))}
              </ul>
            </>
          )}

          {pastas.length > 0 && (
            <>
              <h4>Por pasta</h4>
              <ol className="lista-cobertura-pastas" aria-label="Pastas por lacuna">
                {pastasMostradas.map((p) => (
                  <li key={p.pasta} style={{ background: fundo(p.calor) }}>
                    <code>{p.pasta || "(raiz)"}</code>{" "}
                    {p.lacuna && <span className="badge" data-tone="attention">lacuna</span>}{" "}
                    <span className="muted">
                      referenciada {contar(p.centralidade, "vez", "vezes")} ·{" "}
                      {contar(p.regras, "regra", "regras")} · {contar(p.arquivos, "arquivo", "arquivos")}
                    </span>
                  </li>
                ))}
              </ol>
              {pastas.length > PASTAS_VISIVEIS && (
                <button
                  type="button"
                  className="link"
                  onClick={() => setTodasAsPastas(!todasAsPastas)}
                >
                  {todasAsPastas ? "Mostrar menos" : `Mostrar as ${pastas.length} pastas`}
                </button>
              )}
            </>
          )}
        </>
      )}
    </section>
  );
}
