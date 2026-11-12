import { useCallback, useEffect, useState, type FormEvent } from "react";
import { mensagemDe, type AmbienteClaude, type AnaliseCriada, type Fluxo } from "./analise";
import { ROTULO_FLUXO, type ApiConhecimento } from "./conhecimento";
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
};

/**
 * Formulário de uma análise nova pelos botões do início: o projeto, a entrada do
 * fluxo e, se faltar, o preparo do ambiente do Claude com consentimento.
 */
export function NovaAnalise({
  api,
  fluxo,
  projetos,
  aoCriar,
  aoCancelar,
}: {
  api: ApiConhecimento;
  fluxo: Fluxo;
  projetos: ProjetoResumo[];
  aoCriar: (projeto: string, criada: AnaliseCriada) => void;
  aoCancelar: () => void;
}) {
  const [projeto, setProjeto] = useState(projetos[0]?.id ?? "");
  const [entrada, setEntrada] = useState("");
  const [titulo, setTitulo] = useState("");
  const [issue, setIssue] = useState("");
  const [ambiente, setAmbiente] = useState<AmbienteClaude | null>(null);
  const [criando, setCriando] = useState(false);
  const [falha, setFalha] = useState<string | null>(null);

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
    const deIssue = issue.trim() !== "";
    if (!deIssue && !entrada.trim()) return;
    setCriando(true);
    setFalha(null);
    (deIssue
      ? api.criarAnaliseDeDemanda(projeto, fluxo, issue.trim())
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
      {ambiente && !pronto && (
        <PrepararAmbiente api={api} projeto={projeto} ambiente={ambiente} aoConcluir={verificar} />
      )}
      <label>
        Issue do projeto (opcional)
        <input
          value={issue}
          onChange={(e) => setIssue(e.target.value)}
          placeholder="Ex.: PROJ-123 ou 45"
          spellCheck={false}
        />
      </label>
      {issue.trim() !== "" && (
        <p className="muted">
          O app busca o título e a descrição da issue no repositório de demandas do projeto e guarda
          o link dela na análise.
        </p>
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
          disabled={!pronto || criando || (!entrada.trim() && !issue.trim())}
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
