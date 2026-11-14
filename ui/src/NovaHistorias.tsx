import { useCallback, useEffect, useState, type FormEvent } from "react";
import { mensagemDe, type AmbienteClaude, type AnaliseCriada, type AnaliseResumo } from "./analise";
import { ROTULO_FLUXO, type ApiConhecimento } from "./conhecimento";
import { PrepararAmbiente } from "./PrepararAmbiente";
import type { ProjetoResumo } from "./terminal";

/** Fluxos cujas análises podem ser a origem das histórias: as análises do código. */
const ORIGENS = new Set(["perguntar", "explicar", "impacto"]);

const ESTADO: Record<AnaliseResumo["estado"], string> = {
  criada: "criada",
  "em-andamento": "em andamento",
  concluida: "em revisão",
  aprovada: "aprovada",
};

/**
 * Formulário do fluxo "Histórias e critérios" (#115): o analista escolhe a análise
 * de origem (de preferência aprovada) e, se quiser, diz o que quer nas histórias. O
 * agente lê a origem e registra as histórias com os critérios ligados às afirmações
 * e às regras. Pelo painel da demanda (`demandaFixa`), a origem é opcional: as
 * histórias podem partir só da demanda.
 */
export function NovaHistorias({
  api,
  projetos,
  demandaFixa,
  aoCriar,
  aoCancelar,
}: {
  api: ApiConhecimento;
  projetos: ProjetoResumo[];
  /** Demanda (`D-0001`) do primeiro projeto de `projetos`. */
  demandaFixa?: string;
  aoCriar: (projeto: string, criada: AnaliseCriada) => void;
  aoCancelar: () => void;
}) {
  const [projeto, setProjeto] = useState(projetos[0]?.id ?? "");
  const [analises, setAnalises] = useState<AnaliseResumo[] | null>(null);
  const [origem, setOrigem] = useState("");
  const [entrada, setEntrada] = useState("");
  const [ambiente, setAmbiente] = useState<AmbienteClaude | null>(null);
  const [criando, setCriando] = useState(false);
  const [falha, setFalha] = useState<string | null>(null);

  // Análises do código do projeto: as aprovadas primeiro, depois as mais recentes.
  useEffect(() => {
    if (!projeto) return;
    let ativo = true;
    setAnalises(null);
    setOrigem("");
    api
      .listarAnalises(projeto)
      .then((lista) => {
        if (!ativo) return;
        const candidatas = lista
          .filter((a) => ORIGENS.has(a.fluxo))
          .sort(
            (a, b) =>
              Number(b.estado === "aprovada") - Number(a.estado === "aprovada") ||
              b.atualizadaMs - a.atualizadaMs,
          );
        setAnalises(candidatas);
        setOrigem(candidatas[0]?.id ?? "");
      })
      .catch((e: unknown) => ativo && setFalha(mensagemDe(e)));
    return () => {
      ativo = false;
    };
  }, [api, projeto]);

  const verificar = useCallback(() => {
    if (!projeto) return;
    setAmbiente(null);
    api
      .verificarAmbiente(projeto)
      .then(setAmbiente)
      .catch((e: unknown) => setFalha(mensagemDe(e)));
  }, [api, projeto]);

  useEffect(verificar, [verificar]);

  const escolhida = analises?.find((a) => a.id === origem);
  const podeCriar = !!projeto && (origem !== "" || !!demandaFixa);

  const criar = (ev: FormEvent) => {
    ev.preventDefault();
    if (!podeCriar) return;
    setCriando(true);
    setFalha(null);
    api
      .criarAnaliseDeHistorias(projeto, {
        origem: origem || undefined,
        demanda: demandaFixa,
        entrada: entrada.trim() || undefined,
      })
      .then((criada) => aoCriar(projeto, criada))
      .catch((e: unknown) => {
        setFalha(mensagemDe(e));
        setCriando(false);
      });
  };

  const pronto = ambiente?.pronto === true;

  return (
    <form className="nova-pergunta" onSubmit={criar} aria-label={ROTULO_FLUXO.historias}>
      <h3>{ROTULO_FLUXO.historias}</h3>
      <p className="muted">
        O agente lê a análise de origem e registra histórias "Como / quero / para" com critérios de
        aceite (Dado / Quando / Então). Cada critério fica ligado às afirmações e às regras que o
        sustentam; o que depende de pergunta em aberto fica "a confirmar".
      </p>
      {demandaFixa ? (
        <p className="muted">As histórias ficam ligadas à demanda {demandaFixa}.</p>
      ) : (
        <label>
          Projeto{" "}
          <select value={projeto} onChange={(e) => setProjeto(e.target.value)}>
            {projetos.map((s) => (
              <option key={s.id} value={s.id}>
                {s.nome}
              </option>
            ))}
          </select>
        </label>
      )}
      {ambiente && !pronto && (
        <PrepararAmbiente api={api} projeto={projeto} ambiente={ambiente} aoConcluir={verificar} />
      )}
      <label>
        Análise de origem
        <select
          value={origem}
          onChange={(e) => setOrigem(e.target.value)}
          disabled={analises === null}
        >
          {demandaFixa && <option value="">Só a demanda, sem análise de origem</option>}
          {analises?.map((a) => (
            <option key={a.id} value={a.id}>
              {a.id}: {a.titulo} ({ESTADO[a.estado]})
            </option>
          ))}
        </select>
      </label>
      {analises?.length === 0 && !demandaFixa && (
        <p className="muted">
          Nenhuma análise do código neste projeto ainda. Faça antes a análise de impacto da demanda:
          as histórias partem dela.
        </p>
      )}
      {escolhida && escolhida.estado !== "aprovada" && (
        <p className="muted" role="note">
          A análise {escolhida.id} ainda não foi aprovada: o agente prefere as afirmações que você
          já confirmou e trata as demais como a confirmar.
        </p>
      )}
      <label>
        O que você quer nas histórias (opcional)
        <textarea
          value={entrada}
          onChange={(e) => setEntrada(e.target.value)}
          rows={3}
          placeholder="Ex.: Só o parcelamento na tela; o relatório de recebíveis fica para depois."
        />
      </label>
      <div className="acoes">
        <button
          type="submit"
          className="btn btn-primary"
          disabled={!pronto || criando || !podeCriar}
        >
          Gerar as histórias
        </button>
        <button className="btn" type="button" onClick={aoCancelar}>
          Cancelar
        </button>
      </div>
      {ambiente && !pronto && (
        <p className="muted">Prepare o ambiente do Claude antes de gerar as histórias.</p>
      )}
      {falha && (
        <p className="error-text" role="alert">
          {falha}
        </p>
      )}
    </form>
  );
}
