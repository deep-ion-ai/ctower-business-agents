import { useCallback, useEffect, useState, type FormEvent } from "react";
import {
  apiAnaliseTauri,
  mensagemDe,
  type AmbienteClaude,
  type AnaliseResumo,
  type ApiAnalise,
  type EstadoAnalise,
  type PlanoPreparo,
  type Situacao,
} from "./analise";
import { PainelAnalise, type Abertura } from "./PainelAnalise";
import type { ApiTerminal } from "./terminal";
import type { TelaTerminal } from "./telaXterm";

interface Props {
  /** Para testes. */
  api?: ApiAnalise;
  apiTerminal?: ApiTerminal;
  criarTela?: () => TelaTerminal;
}

const ESTADO: Record<EstadoAnalise, string> = {
  criada: "criada",
  "em-andamento": "em andamento",
  concluida: "concluída",
  aprovada: "aprovada",
};

/**
 * Pergunta ao sistema, do início ao documento: escolher o sistema (com o mínimo do
 * primeiro uso para isso: pasta da base, cadastro e preparo do ambiente com
 * consentimento), perguntar, acompanhar a análise ao vivo e reabrir as anteriores.
 */
export function TelaAnalise({ api = apiAnaliseTauri, apiTerminal, criarTela }: Props) {
  const [situacao, setSituacao] = useState<Situacao | null>(null);
  const [erro, setErro] = useState<string | null>(null);
  const [sistema, setSistema] = useState("");
  const [aberta, setAberta] = useState<{ id: string; abertura: Abertura } | null>(null);

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
      {situacao && !situacao.base && <EscolherBase api={api} aoConcluir={recarregar} />}
      {situacao?.base && situacao.sistemas.length === 0 && (
        <CadastrarSistema api={api} aoConcluir={recarregar} />
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
  aoAbrir,
}: {
  api: ApiAnalise;
  sistema: string;
  aoAbrir: (id: string, abertura: Abertura) => void;
}) {
  const [ambiente, setAmbiente] = useState<AmbienteClaude | null>(null);
  const [falhaAmbiente, setFalhaAmbiente] = useState<string | null>(null);
  const [analises, setAnalises] = useState<AnaliseResumo[] | null>(null);
  const [entrada, setEntrada] = useState("");
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

/**
 * Preparo do ambiente com consentimento: mostra o que falta, o que será alterado na
 * configuração do Claude (sem gravar) e só prepara quando o analista concorda.
 */
export function PrepararAmbiente({
  api,
  sistema,
  ambiente,
  aoConcluir,
}: {
  api: ApiAnalise;
  sistema: string;
  ambiente: AmbienteClaude;
  aoConcluir: () => void;
}) {
  const [plano, setPlano] = useState<PlanoPreparo | null>(null);
  const [ocupado, setOcupado] = useState(false);
  const [falha, setFalha] = useState<string | null>(null);

  const planejar = () => {
    setOcupado(true);
    setFalha(null);
    api
      .planejarPreparo(sistema)
      .then(setPlano)
      .catch((e: unknown) => setFalha(mensagemDe(e)))
      .finally(() => setOcupado(false));
  };

  const preparar = () => {
    setOcupado(true);
    setFalha(null);
    api
      .prepararAmbiente(sistema)
      .then(() => aoConcluir())
      .catch((e: unknown) => setFalha(mensagemDe(e)))
      .finally(() => setOcupado(false));
  };

  const mudancas = plano?.alteracoes.filter((a) => a.acao !== "semMudanca") ?? [];

  return (
    <div className="preparar-ambiente" role="region" aria-label="Preparar o ambiente">
      <p>O ambiente do Claude ainda não está preparado para este sistema:</p>
      <ul>
        {ambiente.pendencias.map((p) => (
          <li key={p}>{p}</li>
        ))}
      </ul>
      {!plano && (
        <button type="button" onClick={planejar} disabled={ocupado}>
          Ver o que será alterado
        </button>
      )}
      {plano && (
        <>
          <p>
            O app vai registrar o servidor MCP e os hooks na configuração do Claude Code. Nada é
            criado nas pastas do sistema, e o app faz uma cópia de segurança de cada arquivo antes
            de gravar.
          </p>
          <ul className="plano-preparo">
            {mudancas.map((a) => (
              <li key={`${a.arquivo}-${a.item.tipo === "hook" ? a.item.evento : a.item.chave}`}>
                {a.item.tipo === "hook"
                  ? `Hook ${a.item.evento}`
                  : `Servidor MCP "cba" para a pasta ${a.item.chave}`}{" "}
                <span className="suave">
                  ({a.acao === "atualizado" ? "atualizado" : "adicionado"} em {a.arquivo})
                </span>
              </li>
            ))}
          </ul>
          <button type="button" onClick={preparar} disabled={ocupado}>
            Concordo, preparar o ambiente
          </button>{" "}
          <button type="button" onClick={() => setPlano(null)} disabled={ocupado}>
            Agora não
          </button>
        </>
      )}
      {falha && (
        <p className="trecho-erro" role="alert">
          {falha}
        </p>
      )}
    </div>
  );
}

function EscolherBase({ api, aoConcluir }: { api: ApiAnalise; aoConcluir: () => void }) {
  const [pasta, setPasta] = useState("");
  const [falha, setFalha] = useState<string | null>(null);
  const salvar = (ev: FormEvent) => {
    ev.preventDefault();
    setFalha(null);
    api
      .configurarBase(pasta)
      .then(() => aoConcluir())
      .catch((e: unknown) => setFalha(mensagemDe(e)));
  };
  return (
    <form onSubmit={salvar} aria-label="Pasta da base de conhecimento">
      <p>
        Escolha a pasta da base de conhecimento, onde ficam os documentos das análises (ex.:
        Documentos/ctower-business). Pode ser uma pasta sincronizada ou um repositório git.
      </p>
      <label>
        Pasta da base (caminho completo){" "}
        <input value={pasta} onChange={(e) => setPasta(e.target.value)} />
      </label>{" "}
      <button type="submit" disabled={!pasta.trim()}>
        Usar esta pasta
      </button>
      {falha && (
        <p className="trecho-erro" role="alert">
          {falha}
        </p>
      )}
    </form>
  );
}

function CadastrarSistema({ api, aoConcluir }: { api: ApiAnalise; aoConcluir: () => void }) {
  const [nome, setNome] = useState("");
  const [pasta, setPasta] = useState("");
  const [falha, setFalha] = useState<string | null>(null);
  const salvar = (ev: FormEvent) => {
    ev.preventDefault();
    setFalha(null);
    api
      .cadastrarSistema(nome, pasta)
      .then(() => aoConcluir())
      .catch((e: unknown) => setFalha(mensagemDe(e)));
  };
  return (
    <form onSubmit={salvar} aria-label="Cadastrar sistema">
      <p>Cadastre o sistema que vai analisar: um nome e a pasta do código.</p>
      <label>
        Nome <input value={nome} onChange={(e) => setNome(e.target.value)} />
      </label>{" "}
      <label>
        Pasta do código (caminho completo){" "}
        <input value={pasta} onChange={(e) => setPasta(e.target.value)} />
      </label>{" "}
      <button type="submit" disabled={!nome.trim() || !pasta.trim()}>
        Cadastrar
      </button>
      {falha && (
        <p className="trecho-erro" role="alert">
          {falha}
        </p>
      )}
    </form>
  );
}
