import { useCallback, useEffect, useState, type FormEvent } from "react";
import { mensagemDe, type AmbienteClaude, type AnaliseCriada, type Fluxo } from "./analise";
import { ROTULO_FLUXO, type ApiConhecimento } from "./conhecimento";
import { PrepararAmbiente } from "./PrepararAmbiente";
import type { SistemaResumo } from "./terminal";

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
    exemplo:
      "Ex.: Permitir parcelamento da fatura em até 3 vezes para clientes pessoa jurídica.",
  },
};

/**
 * Formulário de uma análise nova pelos botões do início: o sistema, a entrada do
 * fluxo e, se faltar, o preparo do ambiente do Claude com consentimento.
 */
export function NovaAnalise({
  api,
  fluxo,
  sistemas,
  aoCriar,
  aoCancelar,
}: {
  api: ApiConhecimento;
  fluxo: Fluxo;
  sistemas: SistemaResumo[];
  aoCriar: (sistema: string, criada: AnaliseCriada) => void;
  aoCancelar: () => void;
}) {
  const [sistema, setSistema] = useState(sistemas[0]?.id ?? "");
  const [entrada, setEntrada] = useState("");
  const [titulo, setTitulo] = useState("");
  const [ambiente, setAmbiente] = useState<AmbienteClaude | null>(null);
  const [criando, setCriando] = useState(false);
  const [falha, setFalha] = useState<string | null>(null);

  const verificar = useCallback(() => {
    if (!sistema) return;
    setAmbiente(null);
    api
      .verificarAmbiente(sistema)
      .then(setAmbiente)
      .catch((e: unknown) => setFalha(mensagemDe(e)));
  }, [api, sistema]);

  useEffect(verificar, [verificar]);

  const criar = (ev: FormEvent) => {
    ev.preventDefault();
    if (!entrada.trim() || !sistema) return;
    setCriando(true);
    setFalha(null);
    api
      .criarAnalise(sistema, fluxo, entrada.trim(), titulo.trim() || undefined)
      .then((criada) => aoCriar(sistema, criada))
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
        Sistema{" "}
        <select value={sistema} onChange={(e) => setSistema(e.target.value)}>
          {sistemas.map((s) => (
            <option key={s.id} value={s.id}>
              {s.nome}
            </option>
          ))}
        </select>
      </label>
      {ambiente && !pronto && (
        <PrepararAmbiente api={api} sistema={sistema} ambiente={ambiente} aoConcluir={verificar} />
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
          className="primario"
          disabled={!pronto || criando || !entrada.trim()}
        >
          Iniciar a análise
        </button>
        <button type="button" onClick={aoCancelar}>
          Cancelar
        </button>
      </div>
      {ambiente && !pronto && (
        <p className="suave">Prepare o ambiente do Claude antes de iniciar a análise.</p>
      )}
      {falha && (
        <p className="trecho-erro" role="alert">
          {falha}
        </p>
      )}
    </form>
  );
}
