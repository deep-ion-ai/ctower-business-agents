import { useCallback, useEffect, useState, type FormEvent } from "react";
import { mensagemDe, type AmbienteClaude, type AnaliseCriada, type AnaliseResumo } from "./analise";
import { ROTULO_FLUXO, type ApiConhecimento } from "./conhecimento";
import { PrepararAmbiente } from "./PrepararAmbiente";
import type { ProjetoResumo } from "./terminal";

/** Fluxos derivados de uma análise de origem: histórias (#115) e cenários (#117). */
export type FluxoDerivado = "historias" | "cenarios";

/**
 * Fluxos cujas análises podem ser a origem: as análises do código e, nos cenários,
 * também a de histórias (os cenários se ligam às histórias).
 */
const ORIGENS: Record<FluxoDerivado, Set<string>> = {
  historias: new Set(["perguntar", "explicar", "impacto"]),
  cenarios: new Set(["perguntar", "explicar", "impacto", "historias"]),
};

const TEXTOS: Record<
  FluxoDerivado,
  {
    explicacao: string;
    ligadas: string;
    semOrigem: string;
    vazio: string;
    pedido: string;
    exemplo: string;
    botao: string;
    preparar: string;
  }
> = {
  historias: {
    explicacao:
      'O agente lê a análise de origem e registra histórias "Como / quero / para" com critérios de aceite (Dado / Quando / Então). Cada critério fica ligado às afirmações e às regras que o sustentam; o que depende de pergunta em aberto fica "a confirmar".',
    ligadas: "As histórias ficam ligadas à demanda",
    semOrigem: "Só a demanda, sem análise de origem",
    vazio:
      "Nenhuma análise do código neste projeto ainda. Faça antes a análise de impacto da demanda: as histórias partem dela.",
    pedido: "O que você quer nas histórias (opcional)",
    exemplo: "Ex.: Só o parcelamento na tela; o relatório de recebíveis fica para depois.",
    botao: "Gerar as histórias",
    preparar: "Prepare o ambiente do Claude antes de gerar as histórias.",
  },
  cenarios: {
    explicacao:
      'O agente lê as regras no código e registra os cenários de teste: tabela de decisão para regra com várias condições, partição de equivalência e valores-limite com os valores lidos do código, e os casos negativos. Cada cenário (CT-…) fica ligado à regra e à evidência do limite; o limite que vem de tabela de parâmetros fica "a confirmar". Exporta em CSV, uma linha por cenário.',
    ligadas: "Os cenários ficam ligados à demanda",
    semOrigem: "Sem análise de origem: só as regras do pedido",
    vazio:
      "Nenhuma análise neste projeto ainda: diga no pedido as regras a cobrir (RN-…).",
    pedido: "Regras ou o que você quer nos cenários",
    exemplo: "Ex.: Cenários do cancelamento: RN-FAT-013 e RN-FAT-014.",
    botao: "Gerar os cenários",
    preparar: "Prepare o ambiente do Claude antes de gerar os cenários.",
  },
};

const ESTADO: Record<AnaliseResumo["estado"], string> = {
  criada: "criada",
  "em-andamento": "em andamento",
  concluida: "em revisão",
  aprovada: "aprovada",
};

/**
 * Formulário dos fluxos derivados de uma análise: "Histórias e critérios" (#115) e
 * "Cenários de teste" (#117). O analista escolhe a análise de origem (de
 * preferência aprovada) e, se quiser, diz o que quer. Pelo painel da demanda
 * (`demandaFixa`), a origem é opcional: o fluxo pode partir só da demanda. Nos
 * cenários, a origem também é opcional fora da demanda, com as regras no pedido.
 */
export function NovaHistorias({
  api,
  projetos,
  demandaFixa,
  aoCriar,
  aoCancelar,
  fluxo = "historias",
}: {
  api: ApiConhecimento;
  projetos: ProjetoResumo[];
  /** Histórias (padrão) ou cenários de teste. */
  fluxo?: FluxoDerivado;
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
          .filter((a) => ORIGENS[fluxo].has(a.fluxo))
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
  }, [api, projeto, fluxo]);

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
  const textos = TEXTOS[fluxo];
  // Nos cenários, sem origem nem demanda, o pedido diz as regras.
  const semOrigemPermitida = !!demandaFixa || fluxo === "cenarios";
  const podeCriar =
    !!projeto &&
    (origem !== "" || !!demandaFixa || (fluxo === "cenarios" && entrada.trim() !== ""));

  const criar = (ev: FormEvent) => {
    ev.preventDefault();
    if (!podeCriar) return;
    setCriando(true);
    setFalha(null);
    const pedido = {
      origem: origem || undefined,
      demanda: demandaFixa,
      entrada: entrada.trim() || undefined,
    };
    (fluxo === "cenarios"
      ? api.criarAnaliseDeCenarios(projeto, pedido)
      : api.criarAnaliseDeHistorias(projeto, pedido)
    )
      .then((criada) => aoCriar(projeto, criada))
      .catch((e: unknown) => {
        setFalha(mensagemDe(e));
        setCriando(false);
      });
  };

  const pronto = ambiente?.pronto === true;

  return (
    <form className="nova-pergunta" onSubmit={criar} aria-label={ROTULO_FLUXO[fluxo]}>
      <h3>{ROTULO_FLUXO[fluxo]}</h3>
      <p className="muted">{textos.explicacao}</p>
      {demandaFixa ? (
        <p className="muted">
          {textos.ligadas} {demandaFixa}.
        </p>
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
          {semOrigemPermitida && (
            <option value="">{demandaFixa ? TEXTOS.historias.semOrigem : textos.semOrigem}</option>
          )}
          {analises?.map((a) => (
            <option key={a.id} value={a.id}>
              {a.id}: {a.titulo} ({ESTADO[a.estado]})
            </option>
          ))}
        </select>
      </label>
      {analises?.length === 0 && !demandaFixa && <p className="muted">{textos.vazio}</p>}
      {escolhida && escolhida.estado !== "aprovada" && (
        <p className="muted" role="note">
          A análise {escolhida.id} ainda não foi aprovada: o agente prefere as afirmações que você
          já confirmou e trata as demais como a confirmar.
        </p>
      )}
      <label>
        {textos.pedido}
        <textarea
          value={entrada}
          onChange={(e) => setEntrada(e.target.value)}
          rows={3}
          placeholder={textos.exemplo}
        />
      </label>
      <div className="acoes">
        <button
          type="submit"
          className="btn btn-primary"
          disabled={!pronto || criando || !podeCriar}
        >
          {textos.botao}
        </button>
        <button className="btn" type="button" onClick={aoCancelar}>
          Cancelar
        </button>
      </div>
      {ambiente && !pronto && (
        <p className="muted">{textos.preparar}</p>
      )}
      {falha && (
        <p className="error-text" role="alert">
          {falha}
        </p>
      )}
    </form>
  );
}
