import { useEffect, useId, useState } from "react";
import { contar } from "./formatacao";
import { mensagemDe } from "./analise";
import type {
  ApiRevisao,
  AprovacaoFeita,
  Area,
  FuncionalidadeResumida,
  OpcoesAprovacao,
  OrigemDoVinculo,
  Pendencia,
  Prontidao,
  RegraPrevista,
} from "./revisao";
import { contarPendencias } from "./resumoRevisao";

/** O que o analista mudou na regra de uma afirmação; `titulo` nulo é o sugerido. */
interface Ajuste {
  area: string;
  titulo: string | null;
  trocarArea: boolean;
}

interface Props {
  api: ApiRevisao;
  projeto: string;
  analise: string;
  /** Versão da análise: a prontidão é relida a cada mudança. */
  versao: number;
  /** Chamado depois de aprovar, para a tela reler a análise. */
  aoAprovar?: (feita: AprovacaoFeita) => void;
  /** "Ir para" de uma pendência com número: o chamador fecha o diálogo e leva ao cartão. */
  aoIrPara?: (pendencia: Pendencia) => void;
  /** "Revisar as N pendentes": o chamador fecha o diálogo e leva à primeira. */
  aoRevisarPendentes?: (pendencias: Pendencia[]) => void;
}

/** Prontidão da aprovação, relida a cada versão da análise; `null` sem o comando. */
export function useProntidao(
  api: ApiRevisao,
  projeto: string,
  analise: string,
  versao: number,
  habilitado = true,
) {
  const [prontidao, setProntidao] = useState<Prontidao | null>(null);
  const [erro, setErro] = useState<string | null>(null);
  useEffect(() => {
    if (!habilitado) return;
    let ativo = true;
    // Sem o comando (ou com resposta inesperada), a aprovação não aparece.
    Promise.resolve()
      .then(() => api.prontidao(projeto, analise))
      .then((p) => {
        if (ativo && p && Array.isArray(p.areas)) setProntidao(p);
      })
      .catch((e: unknown) => ativo && setErro(mensagemDe(e)));
    return () => {
      ativo = false;
    };
  }, [api, projeto, analise, versao, habilitado]);
  return { prontidao, erro, setErro };
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

const EXPLICACAO_DAS_FUNCIONALIDADES: Record<OrigemDoVinculo, string> = {
  documento: "As da aprovação anterior.",
  sessao: "A funcionalidade desta análise.",
  evidencias:
    "Sugeridas pelas evidências (pontos de entrada nos mesmos arquivos): confira antes de aprovar.",
  nenhuma: "Nenhuma sugerida; escolha se a análise tratar de alguma.",
};

/**
 * Funcionalidades do mapa da análise (#287): a proposta vem marcada, e o analista tira ou
 * acrescenta. As regras e os termos herdam.
 */
function EscolhaFuncionalidades({
  escolhidas,
  porId,
  doMapa,
  origem,
  aoMudar,
}: {
  escolhidas: string[];
  porId: Map<string, FuncionalidadeResumida>;
  doMapa: FuncionalidadeResumida[];
  origem: OrigemDoVinculo;
  aoMudar: (ids: string[]) => void;
}) {
  const id = useId();
  const disponiveis = doMapa.filter((f) => !escolhidas.includes(f.id));
  return (
    <div className="funcionalidades-aprovacao" role="group" aria-label="Funcionalidades do mapa">
      <p className="muted">
        <strong>Funcionalidades do mapa</strong> (as regras e os termos herdam).{" "}
        {EXPLICACAO_DAS_FUNCIONALIDADES[origem]}
      </p>
      {escolhidas.length > 0 && (
        <ul className="chips-funcionalidades" aria-label="Funcionalidades vinculadas">
          {escolhidas.map((f) => (
            <li key={f} className="chip-funcionalidade">
              <span>
                {f}
                {porId.get(f) ? ` · ${porId.get(f)!.nome}` : ""}
              </span>
              <button
                type="button"
                aria-label={`Tirar a funcionalidade ${f}`}
                onClick={() => aoMudar(escolhidas.filter((x) => x !== f))}
              >
                ×
              </button>
            </li>
          ))}
        </ul>
      )}
      {disponiveis.length > 0 && (
        <span className="campo-area">
          <label htmlFor={id}>Acrescentar</label>{" "}
          <select
            id={id}
            value=""
            onChange={(e) => e.target.value && aoMudar([...escolhidas, e.target.value])}
          >
            <option value="">Escolha a funcionalidade</option>
            {disponiveis.map((f) => (
              <option key={f.id} value={f.id}>
                {f.id} · {f.nome} ({f.areaNome})
              </option>
            ))}
          </select>
        </span>
      )}
    </div>
  );
}

/**
 * Aprovação da análise (#37, spec 7.4; diálogo guiado, #186): mostra o que falta revisar
 * (com "Ir para" em cada item), pede a área das regras (obrigatória quando o projeto
 * declara mais de uma) e, ao aprovar, leva à base as afirmações confirmadas marcadas
 * para virar regra (#214: respostas e exemplos vêm desmarcados) e os termos aceitos
 * (glossário). As incorretas ficam registradas e não viram regra.
 */
export function AprovacaoAnalise({
  api,
  projeto,
  analise,
  versao,
  aoAprovar,
  aoIrPara,
  aoRevisarPendentes,
}: Props) {
  const { prontidao, erro, setErro } = useProntidao(api, projeto, analise, versao);
  const [area, setArea] = useState("");
  const [ajustes, setAjustes] = useState<Record<number, Ajuste>>({});
  const [marcadas, setMarcadas] = useState<Record<number, boolean>>({});
  // Funcionalidades escolhidas pelo analista; `null` são as propostas.
  const [funcionalidades, setFuncionalidades] = useState<string[] | null>(null);
  const [ocupado, setOcupado] = useState(false);
  const [feita, setFeita] = useState<AprovacaoFeita | null>(null);

  // Com uma área só, ela já vem escolhida.
  const unica = prontidao?.areas.length === 1 ? prontidao.areas[0].id : "";
  useEffect(() => {
    if (unica) setArea((a) => a || unica);
  }, [unica]);

  if (!prontidao) return erro ? <p className="error-text">{erro}</p> : null;

  // Vínculo com o mapa (#287): só quando o app manda a proposta.
  const comVinculo = Array.isArray(prontidao.funcionalidades);
  const porId = new Map(
    [...(prontidao.funcionalidadesDoMapa ?? []), ...(prontidao.funcionalidades ?? [])].map((f) => [
      f.id,
      f,
    ]),
  );
  const vinculadas = funcionalidades ?? (prontidao.funcionalidades ?? []).map((f) => f.id);
  const areaDaPrimeira = () => {
    const f = vinculadas.map((id) => porId.get(id)).find(Boolean);
    if (!f) return "";
    // A área precisa estar no `projeto.md` (quando ele declara alguma) para a escolha.
    return prontidao.areas.length === 0 || prontidao.areas.some((a) => a.id === f.area) ? f.area : "";
  };
  // Sem a escolha do analista, a área da funcionalidade no lugar de `geral`.
  const areaSugerida = funcionalidades === null ? (prontidao.areaSugerida ?? "") : areaDaPrimeira();
  const areaEfetiva = area || areaSugerida;

  const ajuste = (n: number): Ajuste => ajustes[n] ?? { area: "", titulo: null, trocarArea: false };
  const mudar = (n: number, campo: Partial<Ajuste>) =>
    setAjustes((a) => ({ ...a, [n]: { ...ajuste(n), ...campo } }));
  // Sem a escolha do analista, vale a sugestão: respostas e exemplos ficam de fora.
  const marcada = (r: RegraPrevista) => marcadas[r.afirmacao] ?? r.sugerida !== false;
  const escolhidas = prontidao.regras.filter(marcada);
  const todasComArea = escolhidas.length > 0 && escolhidas.every((r) => ajuste(r.afirmacao).area);
  // Sem regras a criar (as histórias, por exemplo), a área não importa.
  const faltaArea =
    prontidao.areaObrigatoria && escolhidas.length > 0 && !areaEfetiva && !todasComArea;
  const aprovada = prontidao.estado === "aprovada";
  const podeGravar = prontidao.pendencias.length === 0;
  // A mesma conta do cabeçalho e da faixa (#222).
  const pendentes = contarPendencias(prontidao.pendencias);

  const aprovar = () => {
    const opcoes: OpcoesAprovacao = {
      area: areaEfetiva.trim() || null,
      afirmacoes: {},
      regras: escolhidas.map((r) => r.afirmacao),
    };
    if (comVinculo) opcoes.funcionalidades = vinculadas;
    for (const r of escolhidas) {
      const a = ajuste(r.afirmacao);
      const titulo = a.titulo?.trim() ?? "";
      const trocado = titulo && titulo !== r.titulo ? titulo : null;
      if (a.area.trim() || trocado)
        opcoes.afirmacoes[r.afirmacao] = {
          area: a.area.trim() || null,
          titulo: trocado,
        };
    }
    setOcupado(true);
    setErro(null);
    api
      .aprovar(projeto, analise, opcoes)
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
              <li key={`${p.tipo}-${p.numero ?? ""}`}>
                {p.motivo}
                {aoIrPara && p.tipo === "afirmacao" && p.numero !== null && (
                  <>
                    {" "}
                    <button
                      className="btn btn-link"
                      type="button"
                      aria-label={`Ir para: ${p.motivo}`}
                      onClick={() => aoIrPara(p)}
                    >
                      Ir para
                    </button>
                  </>
                )}
              </li>
            ))}
          </ul>
          {aoRevisarPendentes && pendentes > 0 && (
            <button
              className="btn"
              type="button"
              onClick={() => aoRevisarPendentes(prontidao.pendencias)}
            >
              {pendentes === 1 ? "Revisar a pendente" : `Revisar as ${pendentes} pendentes`}
            </button>
          )}
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
              : `${escolhidas.length} de ${contar(prontidao.regras.length, "afirmação confirmada", "afirmações confirmadas")} vão virar regra.`}
            {prontidao.incorretas.length > 0 &&
              ` ${contar(prontidao.incorretas.length, "incorreta", "incorretas")} ${prontidao.incorretas.length === 1 ? "fica registrada e não vira regra." : "ficam registradas e não viram regra."}`}
            {prontidao.termosAceitos.length > 0 &&
              ` Termos para o glossário: ${prontidao.termosAceitos.join(", ")}.`}
          </p>
          {comVinculo && (
            <EscolhaFuncionalidades
              escolhidas={vinculadas}
              porId={porId}
              doMapa={prontidao.funcionalidadesDoMapa ?? []}
              origem={
                funcionalidades === null
                  ? (prontidao.origemDasFuncionalidades ?? "nenhuma")
                  : vinculadas.length > 0
                    ? "documento"
                    : "nenhuma"
              }
              aoMudar={setFuncionalidades}
            />
          )}
          <SeletorArea
            areas={prontidao.areas}
            valor={areaEfetiva}
            rotulo="Área das regras"
            vazio={prontidao.areas.length > 0 ? "Escolha a área" : "Área (opcional)"}
            aoMudar={setArea}
          />
          {prontidao.regras.length > 0 && (
            <>
              <p className="muted">
                Marque o que vira regra. Respostas e exemplos vêm desmarcados.
              </p>
              <ol className="regras-previstas" aria-label="Afirmações confirmadas">
                {prontidao.regras.map((r) => {
                  const a = ajuste(r.afirmacao);
                  const sim = marcada(r);
                  return (
                    <li key={r.afirmacao} className="regra-prevista">
                      <label className="regra-escolha">
                        <input
                          type="checkbox"
                          checked={sim}
                          aria-label={`A afirmação ${r.afirmacao} vira regra`}
                          onChange={(e) =>
                            setMarcadas((m) => ({
                              ...m,
                              [r.afirmacao]: e.target.checked,
                            }))
                          }
                        />{" "}
                        <span>
                          Afirmação {r.afirmacao}
                          {r.secaoTitulo ? ` · ${r.secaoTitulo}` : ""}
                        </span>
                      </label>
                      <p className="regra-texto muted">{r.texto}</p>
                      {sim && (
                        <div className="regra-campos">
                          <label>
                            <span>Título</span>
                            <input
                              aria-label={`Título da regra da afirmação ${r.afirmacao}`}
                              value={a.titulo ?? r.titulo}
                              placeholder={r.titulo}
                              onChange={(e) => mudar(r.afirmacao, { titulo: e.target.value })}
                            />
                          </label>
                          {a.trocarArea || a.area ? (
                            <SeletorArea
                              areas={prontidao.areas}
                              valor={a.area}
                              rotulo={`Área da afirmação ${r.afirmacao}`}
                              vazio="A das regras"
                              aoMudar={(v) => mudar(r.afirmacao, { area: v })}
                            />
                          ) : (
                            <button
                              className="btn btn-link"
                              type="button"
                              aria-label={`Trocar a área da afirmação ${r.afirmacao}`}
                              onClick={() => mudar(r.afirmacao, { trocarArea: true })}
                            >
                              Trocar a área
                            </button>
                          )}
                        </div>
                      )}
                    </li>
                  );
                })}
              </ol>
            </>
          )}
          {faltaArea && prontidao.podeAprovar && (
            <p className="muted">O projeto tem mais de uma área: escolha a das regras.</p>
          )}
          <button
            className="btn"
            type="button"
            disabled={!prontidao.podeAprovar || faltaArea || ocupado}
            onClick={aprovar}
          >
            {podeGravar ? "Aprovar e gravar na base" : "Aprovar…"}
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
          {feita.funcionalidades && feita.funcionalidades.length > 0 && (
            <p>Funcionalidades do mapa: {feita.funcionalidades.join(", ")}</p>
          )}
          {feita.documentoNegocial && (
            <p>Documento negocial no mapa de funcionalidades: {feita.documentoNegocial}</p>
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
