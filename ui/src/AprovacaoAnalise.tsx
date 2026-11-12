import { useEffect, useId, useState } from "react";
import { mensagemDe } from "./analise";
import type { ApiRevisao, AprovacaoFeita, Area, OpcoesAprovacao, Prontidao } from "./revisao";

interface Props {
  api: ApiRevisao;
  sistema: string;
  analise: string;
  /** Versão da análise: a prontidão é relida a cada mudança. */
  versao: number;
  /** Chamado depois de aprovar, para a tela reler a análise. */
  aoAprovar?: (feita: AprovacaoFeita) => void;
}

const ACAO = { criada: "criada", atualizada: "atualizada", "sem-mudanca": "sem mudança" } as const;

function SeletorArea({
  areas,
  valor,
  vazio,
  rotulo,
  aoMudar,
}: {
  areas: Area[];
  valor: string;
  vazio: string;
  rotulo: string;
  aoMudar: (v: string) => void;
}) {
  const id = useId();
  return (
    <span className="campo-area">
      <label htmlFor={id}>{rotulo}</label>{" "}
      {areas.length > 0 ? (
        <select id={id} value={valor} onChange={(e) => aoMudar(e.target.value)}>
          <option value="">{vazio}</option>
          {areas.map((a) => (
            <option key={a.id} value={a.id}>
              {a.nome}
            </option>
          ))}
        </select>
      ) : (
        <input id={id} value={valor} placeholder={vazio} onChange={(e) => aoMudar(e.target.value)} />
      )}
    </span>
  );
}

/**
 * Aprovação da análise (#37, spec 7.4): mostra o que falta revisar, pede a área das
 * regras (obrigatória quando o sistema declara mais de uma) e, ao aprovar, leva à
 * base as afirmações confirmadas (regras) e os termos aceitos (glossário). As
 * incorretas ficam registradas na análise e não viram regra.
 */
export function AprovacaoAnalise({ api, sistema, analise, versao, aoAprovar }: Props) {
  const [prontidao, setProntidao] = useState<Prontidao | null>(null);
  const [erro, setErro] = useState<string | null>(null);
  const [area, setArea] = useState("");
  const [ajustes, setAjustes] = useState<Record<number, { area: string; titulo: string }>>({});
  const [ocupado, setOcupado] = useState(false);
  const [feita, setFeita] = useState<AprovacaoFeita | null>(null);

  useEffect(() => {
    let ativo = true;
    // Sem o comando (ou com resposta inesperada), a aprovação não aparece.
    Promise.resolve()
      .then(() => api.prontidao(sistema, analise))
      .then((p) => {
        if (!ativo || !p || !Array.isArray(p.areas)) return;
        setProntidao(p);
        // Com uma área só, ela já vem escolhida.
        if (p.areas.length === 1) setArea((a) => a || p.areas[0].id);
      })
      .catch((e: unknown) => ativo && setErro(mensagemDe(e)));
    return () => {
      ativo = false;
    };
  }, [api, sistema, analise, versao]);

  if (!prontidao) return erro ? <p className="error-text">{erro}</p> : null;

  const ajuste = (n: number) => ajustes[n] ?? { area: "", titulo: "" };
  const mudar = (n: number, campo: "area" | "titulo", valor: string) =>
    setAjustes((a) => ({ ...a, [n]: { ...ajuste(n), [campo]: valor } }));
  const todasComArea =
    prontidao.regras.length > 0 && prontidao.regras.every((r) => ajuste(r.afirmacao).area);
  const faltaArea = prontidao.areaObrigatoria && !area && !todasComArea;
  const aprovada = prontidao.estado === "aprovada";

  const aprovar = () => {
    const opcoes: OpcoesAprovacao = { area: area.trim() || null, afirmacoes: {} };
    for (const r of prontidao.regras) {
      const a = ajuste(r.afirmacao);
      if (a.area || a.titulo.trim())
        opcoes.afirmacoes[r.afirmacao] = {
          area: a.area.trim() || null,
          titulo: a.titulo.trim() || null,
        };
    }
    setOcupado(true);
    setErro(null);
    api
      .aprovar(sistema, analise, opcoes)
      .then((f) => {
        setFeita(f);
        aoAprovar?.(f);
      })
      .catch((e: unknown) => setErro(mensagemDe(e)))
      .finally(() => setOcupado(false));
  };

  return (
    <section className="aprovacao" aria-label="Aprovação">
      <h3>Aprovação</h3>
      {aprovada && !feita && <p role="status">Análise aprovada: as regras já estão na base.</p>}
      {!aprovada && prontidao.pendencias.length > 0 && (
        <div className="pendencias">
          <p>Antes de aprovar:</p>
          <ul aria-label="Pendências">
            {prontidao.pendencias.map((p) => (
              <li key={`${p.tipo}-${p.numero ?? ""}`}>{p.motivo}</li>
            ))}
          </ul>
        </div>
      )}
      {!aprovada && prontidao.avisos.length > 0 && (
        <ul className="muted" aria-label="Avisos da aprovação">
          {prontidao.avisos.map((a) => (
            <li key={a}>{a}</li>
          ))}
        </ul>
      )}
      {!aprovada && (
        <>
          <p className="muted">
            {prontidao.regras.length === 0
              ? "Nenhuma afirmação confirmada: a aprovação não cria regras."
              : `${prontidao.regras.length} afirmação(ões) confirmada(s) vão virar regra.`}
            {prontidao.incorretas.length > 0 &&
              ` ${prontidao.incorretas.length} incorreta(s) ficam registradas e não viram regra.`}
            {prontidao.termosAceitos.length > 0 &&
              ` Termos para o glossário: ${prontidao.termosAceitos.join(", ")}.`}
          </p>
          <SeletorArea
            areas={prontidao.areas}
            valor={area}
            rotulo="Área das regras"
            vazio={prontidao.areas.length > 0 ? "Escolha a área" : "Área (opcional)"}
            aoMudar={setArea}
          />
          {prontidao.regras.length > 0 && (
            <ol className="regras-previstas" aria-label="Regras que a aprovação cria">
              {prontidao.regras.map((r) => (
                <li key={r.afirmacao}>
                  <label>
                    Título da regra da afirmação {r.afirmacao}{" "}
                    <input
                      value={ajuste(r.afirmacao).titulo}
                      placeholder={r.titulo}
                      onChange={(e) => mudar(r.afirmacao, "titulo", e.target.value)}
                    />
                  </label>{" "}
                  <SeletorArea
                    areas={prontidao.areas}
                    valor={ajuste(r.afirmacao).area}
                    rotulo={`Área da afirmação ${r.afirmacao}`}
                    vazio="A da análise"
                    aoMudar={(v) => mudar(r.afirmacao, "area", v)}
                  />
                </li>
              ))}
            </ol>
          )}
          {faltaArea && prontidao.podeAprovar && (
            <p className="muted">O sistema tem mais de uma área: escolha a das regras.</p>
          )}
          <button
            className="btn"
            type="button"
            disabled={!prontidao.podeAprovar || faltaArea || ocupado}
            onClick={aprovar}
          >
            Aprovar análise
          </button>
        </>
      )}
      {erro && (
        <p className="error-text" role="alert">
          {erro}
        </p>
      )}
      {feita && (
        <div role="status" aria-label="Resultado da aprovação">
          <p>{feita.jaAprovada ? "A base foi conferida." : "Análise aprovada."}</p>
          {feita.regras.length > 0 && (
            <ul aria-label="Regras na base">
              {feita.regras.map((r) => (
                <li key={r.id}>
                  {r.id} ({r.area}): {ACAO[r.acao]}
                </li>
              ))}
            </ul>
          )}
          {feita.termos.length > 0 && (
            <ul aria-label="Termos no glossário">
              {feita.termos.map((t) => (
                <li key={t.arquivo}>
                  {t.termo}: {ACAO[t.acao]}
                </li>
              ))}
            </ul>
          )}
          {feita.avisos.map((a) => (
            <p key={a} className="muted">
              {a}
            </p>
          ))}
        </div>
      )}
    </section>
  );
}
