import { useCallback, useEffect, useState, type FormEvent } from "react";
import { mensagemDe, type AmbienteClaude, type AnaliseCriada, type Fluxo } from "./analise";
import { ROTULO_FLUXO, type ApiConhecimento } from "./conhecimento";
import type { DemandaVista } from "./demandas";
import { PrepararAmbiente } from "./PrepararAmbiente";
import type { ProjetoResumo } from "./terminal";

/** Rótulo e exemplo da entrada de cada fluxo (spec, seções 4.3 a 4.5). */
const ENTRADA: Record<Fluxo, { rotulo: string; exemplo: string }> = {
  perguntar: {
    rotulo: "Pergunta",
    exemplo: "Ex.: Como é calculada a multa por atraso de pagamento?",
  },
  explicar: {
    rotulo: "Funcionalidade",
    exemplo: "Ex.: Cancelamento de fatura",
  },
  impacto: {
    rotulo: "Texto da demanda",
    exemplo: "Ex.: Permitir parcelamento da fatura em até 3 vezes para clientes pessoa jurídica.",
  },
  // A reunião tem formulário próprio (NovaReuniao), com o transcript.
  reuniao: { rotulo: "Transcript", exemplo: "" },
  revisar: {
    rotulo: "Requisito ou história a revisar",
    exemplo:
      "Cole o texto como o autor escreveu, com os critérios de aceite, ou informe a issue acima.",
  },
  // Histórias e cenários têm formulário próprio (NovaHistorias), com a análise de origem.
  historias: { rotulo: "Pedido", exemplo: "" },
  cenarios: { rotulo: "Pedido", exemplo: "" },
};

/**
 * Formulário de uma análise nova pelos botões do início: o projeto, a entrada do
 * fluxo, a demanda aberta a que ligar (opcional) e, se faltar, o preparo do
 * ambiente do Claude com consentimento. Pelo painel da demanda, `demandaFixa` liga
 * a análise àquela demanda, sem escolha de projeto, issue nem demanda.
 */
export function NovaAnalise({
  api,
  fluxo,
  projetos,
  demandaFixa,
  aoCriar,
  aoCancelar,
}: {
  api: ApiConhecimento;
  fluxo: Fluxo;
  projetos: ProjetoResumo[];
  /** Demanda (`D-0001`) do primeiro projeto de `projetos`. */
  demandaFixa?: string;
  aoCriar: (projeto: string, criada: AnaliseCriada) => void;
  aoCancelar: () => void;
}) {
  const [projeto, setProjeto] = useState(projetos[0]?.id ?? "");
  const [entrada, setEntrada] = useState("");
  const [titulo, setTitulo] = useState("");
  const [issue, setIssue] = useState("");
  const [abertas, setAbertas] = useState<DemandaVista[]>([]);
  const [demanda, setDemanda] = useState(demandaFixa ?? "");
  const [ambiente, setAmbiente] = useState<AmbienteClaude | null>(null);
  const [criando, setCriando] = useState(false);
  const [falha, setFalha] = useState<string | null>(null);

  // Demandas abertas do projeto, para ligar a análise (opcional).
  useEffect(() => {
    if (demandaFixa || !projeto) return;
    let ativo = true;
    setDemanda("");
    setAbertas([]);
    api
      .listarDemandas(projeto)
      .then((l) => ativo && setAbertas(l.filter((d) => d.status === "aberta")))
      // Sem a lista, a análise só não pode ser ligada a uma demanda.
      .catch(() => undefined);
    return () => {
      ativo = false;
    };
  }, [api, projeto, demandaFixa]);

  const verificar = useCallback(() => {
    if (!projeto) return;
    setAmbiente(null);
    api
      .verificarAmbiente(projeto)
      .then(setAmbiente)
      .catch((e: unknown) => setFalha(mensagemDe(e)));
  }, [api, projeto]);

  useEffect(verificar, [verificar]);

  const criar = (ev: FormEvent) => {
    ev.preventDefault();
    if (!projeto) return;
    // Com a issue informada, o título e a descrição dela são a entrada.
    const deIssue = !demandaFixa && issue.trim() !== "";
    if (!deIssue && !entrada.trim()) return;
    setCriando(true);
    setFalha(null);
    (deIssue
      ? api.criarAnaliseDeDemanda(projeto, fluxo, issue.trim())
      : demanda
        ? api.criarAnalise(projeto, fluxo, entrada.trim(), titulo.trim() || undefined, demanda)
        : api.criarAnalise(projeto, fluxo, entrada.trim(), titulo.trim() || undefined)
    )
      .then((criada) => aoCriar(projeto, criada))
      .catch((e: unknown) => {
        setFalha(mensagemDe(e));
        setCriando(false);
      });
  };

  const pronto = ambiente?.pronto === true;
  const { rotulo, exemplo } = ENTRADA[fluxo];

  return (
    <form className="nova-pergunta" onSubmit={criar} aria-label={ROTULO_FLUXO[fluxo]}>
      <h3>{ROTULO_FLUXO[fluxo]}</h3>
      {demandaFixa ? (
        <p className="muted">A análise fica ligada à demanda {demandaFixa}.</p>
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
      {!demandaFixa && (
        <label>
          {fluxo === "revisar" ? "Chave no rastreador (opcional)" : "Issue do projeto (opcional)"}
          <input
            value={issue}
            onChange={(e) => setIssue(e.target.value)}
            placeholder="Ex.: PROJ-123 ou 45"
            spellCheck={false}
          />
        </label>
      )}
      {!demandaFixa && issue.trim() !== "" && (
        <p className="muted">
          O app busca o título e a descrição da issue no repositório de demandas do projeto e guarda
          o link dela na análise.
        </p>
      )}
      {!demandaFixa && issue.trim() === "" && abertas.length > 0 && (
        <label>
          Demanda (opcional)
          <select value={demanda} onChange={(e) => setDemanda(e.target.value)}>
            <option value="">Sem demanda</option>
            {abertas.map((d) => (
              <option key={d.id} value={d.id}>
                {d.id}: {d.titulo}
              </option>
            ))}
          </select>
        </label>
      )}
      <label>
        {rotulo}
        <textarea
          value={entrada}
          onChange={(e) => setEntrada(e.target.value)}
          rows={3}
          placeholder={exemplo}
        />
      </label>
      <label>
        Título (opcional)
        <input value={titulo} onChange={(e) => setTitulo(e.target.value)} />
      </label>
      <div className="acoes">
        <button
          type="submit"
          className="btn btn-primary"
          disabled={!pronto || criando || (!entrada.trim() && !(issue.trim() && !demandaFixa))}
        >
          Iniciar a análise
        </button>
        <button className="btn" type="button" onClick={aoCancelar}>
          Cancelar
        </button>
      </div>
      {ambiente && !pronto && (
        <p className="muted">Prepare o ambiente do Claude antes de iniciar a análise.</p>
      )}
      {falha && (
        <p className="error-text" role="alert">
          {falha}
        </p>
      )}
    </form>
  );
}
