import { useCallback, useEffect, useState, type FormEvent } from "react";
import {
  apiAnaliseTauri,
  mensagemDe,
  type AmbienteClaude,
  type AnaliseResumo,
  type ApiAnalise,
  type EstadoAnalise,
  type Situacao,
} from "./analise";
import { PainelAnalise, type Abertura } from "./PainelAnalise";
import { PrepararAmbiente } from "./PrepararAmbiente";
import type { ApiTerminal } from "./terminal";
import type { TelaTerminal } from "./telaXterm";

interface Props {
  /** Para testes. */
  api?: ApiAnalise;
  apiTerminal?: ApiTerminal;
  criarTela?: () => TelaTerminal;
  /** Sistema escolhido ao abrir (ex.: o recém-cadastrado no primeiro uso). */
  sistemaInicial?: string;
  /** Pergunta já preenchida (a sugestão do fim do primeiro uso). */
  perguntaInicial?: string;
  /** Abre o assistente do primeiro uso quando falta a base ou um sistema. */
  aoPrimeiroUso?: () => void;
  /** Análise já aberta (criada pelo início ou escolhida numa lista); exige `sistemaInicial`. */
  aberturaInicial?: { id: string; abertura: Abertura };
}

const ESTADO: Record<EstadoAnalise, string> = {
  criada: "criada",
  "em-andamento": "em andamento",
  concluida: "concluída",
  aprovada: "aprovada",
};

/**
 * Pergunta ao sistema, do início ao documento: escolher o sistema (preparando o
 * ambiente com consentimento, se faltar), perguntar, acompanhar a análise ao vivo e
 * reabrir as anteriores. A base e o cadastro ficam no primeiro uso.
 */
export function TelaAnalise({
  api = apiAnaliseTauri,
  apiTerminal,
  criarTela,
  sistemaInicial = "",
  perguntaInicial = "",
  aoPrimeiroUso,
  aberturaInicial,
}: Props) {
  const [situacao, setSituacao] = useState<Situacao | null>(null);
  const [erro, setErro] = useState<string | null>(null);
  const [sistema, setSistema] = useState(sistemaInicial);
  const [aberta, setAberta] = useState<{ id: string; abertura: Abertura } | null>(
    aberturaInicial ?? null,
  );

  const recarregar = useCallback(() => {
    api
      .situacao()
      .then((s) => {
        setSituacao(s);
        setErro(null);
        setSistema((atual) =>
          s.sistemas.some((x) => x.id === atual) ? atual : (s.sistemas[0]?.id ?? ""),
        );
      })
      .catch((e: unknown) => setErro(mensagemDe(e)));
  }, [api]);

  useEffect(recarregar, [recarregar]);

  if (aberta && sistema) {
    return (
      <section className="tela-analise" aria-label="Análise">
        <PainelAnalise
          key={aberta.id}
          api={api}
          sistema={sistema}
          analise={aberta.id}
          abertura={aberta.abertura}
          aoVoltar={() => setAberta(null)}
          apiTerminal={apiTerminal}
          criarTela={criarTela}
        />
      </section>
    );
  }

  return (
    <section className="tela-analise" aria-label="Pergunta ao sistema">
      <h2>Pergunta ao sistema</h2>
      {erro && (
        <p className="trecho-erro" role="alert">
          {erro}
        </p>
      )}
      {!situacao && !erro && <p className="suave">Carregando…</p>}
      {situacao && (!situacao.base || situacao.sistemas.length === 0) && (
        <p>
          {!situacao.base
            ? "Falta escolher a pasta da base de conhecimento."
            : "Nenhum sistema cadastrado."}{" "}
          {aoPrimeiroUso && (
            <button type="button" onClick={aoPrimeiroUso}>
              Abrir o primeiro uso
            </button>
          )}
        </p>
      )}
      {situacao?.base && situacao.sistemas.length > 0 && (
        <>
          <label>
            Sistema{" "}
            <select value={sistema} onChange={(e) => setSistema(e.target.value)}>
              {situacao.sistemas.map((s) => (
                <option key={s.id} value={s.id}>
                  {s.nome}
                </option>
              ))}
            </select>
          </label>
          {sistema && (
            <Sistema
              key={sistema}
              api={api}
              sistema={sistema}
              perguntaInicial={perguntaInicial}
              aoAbrir={(id, abertura) => setAberta({ id, abertura })}
            />
          )}
        </>
      )}
    </section>
  );
}

function Sistema({
  api,
  sistema,
  perguntaInicial,
  aoAbrir,
}: {
  api: ApiAnalise;
  sistema: string;
  perguntaInicial: string;
  aoAbrir: (id: string, abertura: Abertura) => void;
}) {
  const [ambiente, setAmbiente] = useState<AmbienteClaude | null>(null);
  const [falhaAmbiente, setFalhaAmbiente] = useState<string | null>(null);
  const [analises, setAnalises] = useState<AnaliseResumo[] | null>(null);
  const [entrada, setEntrada] = useState(perguntaInicial);
  const [titulo, setTitulo] = useState("");
  const [criando, setCriando] = useState(false);
  const [falha, setFalha] = useState<string | null>(null);

  const verificar = useCallback(() => {
    setFalhaAmbiente(null);
    api
      .verificarAmbiente(sistema)
      .then(setAmbiente)
      .catch((e: unknown) => setFalhaAmbiente(mensagemDe(e)));
  }, [api, sistema]);

  useEffect(verificar, [verificar]);

  useEffect(() => {
    let ativo = true;
    api
      .listarAnalises(sistema)
      .then((l) => ativo && setAnalises(l))
      .catch((e: unknown) => ativo && setFalha(mensagemDe(e)));
    return () => {
      ativo = false;
    };
  }, [api, sistema]);

  const perguntar = (ev: FormEvent) => {
    ev.preventDefault();
    if (!entrada.trim()) return;
    setCriando(true);
    setFalha(null);
    api
      .criarAnalise(sistema, entrada.trim(), titulo.trim() || undefined)
      .then((criada) =>
        aoAbrir(criada.analise.id, { tipo: "nova", promptInicial: criada.promptInicial }),
      )
      .catch((e: unknown) => {
        setFalha(mensagemDe(e));
        setCriando(false);
      });
  };

  const pronto = ambiente?.pronto === true;

  return (
    <>
      {falhaAmbiente && (
        <p className="trecho-erro" role="alert">
          Não foi possível verificar o ambiente do Claude: {falhaAmbiente}
        </p>
      )}
      {ambiente && !pronto && (
        <PrepararAmbiente api={api} sistema={sistema} ambiente={ambiente} aoConcluir={verificar} />
      )}

      <form className="nova-pergunta" onSubmit={perguntar} aria-label="Nova pergunta">
        <label>
          Pergunta
          <textarea
            value={entrada}
            onChange={(e) => setEntrada(e.target.value)}
            rows={3}
            placeholder="Ex.: Como é calculada a multa por atraso de pagamento?"
          />
        </label>
        <label>
          Título (opcional)
          <input value={titulo} onChange={(e) => setTitulo(e.target.value)} />
        </label>
        <button type="submit" disabled={!pronto || criando || !entrada.trim()}>
          Perguntar
        </button>
        {!pronto && ambiente && (
          <span className="suave"> Prepare o ambiente do Claude antes de perguntar.</span>
        )}
      </form>
      {falha && (
        <p className="trecho-erro" role="alert">
          {falha}
        </p>
      )}

      <section aria-label="Análises do sistema">
        <h3>Análises</h3>
        {analises?.length === 0 && <p className="suave">Nenhuma análise ainda.</p>}
        {analises && analises.length > 0 && (
          <ul className="lista-analises">
            {analises.map((a) => (
              <li key={a.id}>
                <button type="button" onClick={() => aoAbrir(a.id, { tipo: "reabrir" })}>
                  {a.id}: {a.titulo}
                </button>{" "}
                <span className="suave">{ESTADO[a.estado]}</span>
              </li>
            ))}
          </ul>
        )}
      </section>
    </>
  );
}
