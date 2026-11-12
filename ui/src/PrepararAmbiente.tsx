import { useCallback, useEffect, useState } from "react";
import { mensagemDe, type AmbienteClaude, type ApiAmbiente, type PlanoPreparo } from "./analise";

/**
 * Preparo do ambiente com consentimento: mostra o que falta, o que será alterado na
 * configuração do Claude (sem gravar) e só prepara quando o analista concorda.
 */
export function PrepararAmbiente({
  api,
  projeto,
  ambiente,
  aoConcluir,
}: {
  api: ApiAmbiente;
  projeto: string;
  ambiente: AmbienteClaude;
  aoConcluir: () => void;
}) {
  const [plano, setPlano] = useState<PlanoPreparo | null>(null);
  const [ocupado, setOcupado] = useState(false);
  const [falha, setFalha] = useState<string | null>(null);
  // Proteção opcional contra edição (#29): desligada por padrão.
  const [proteger, setProteger] = useState(false);

  const planejar = () => {
    setOcupado(true);
    setFalha(null);
    api
      .planejarPreparo(projeto)
      .then(setPlano)
      .catch((e: unknown) => setFalha(mensagemDe(e)))
      .finally(() => setOcupado(false));
  };

  const preparar = () => {
    setOcupado(true);
    setFalha(null);
    api
      .prepararAmbiente(projeto)
      .then(() => (proteger && api.mudarProtecao ? api.mudarProtecao(projeto, true) : undefined))
      .then(() => aoConcluir())
      .catch((e: unknown) => setFalha(mensagemDe(e)))
      .finally(() => setOcupado(false));
  };

  const mudancas = plano?.alteracoes.filter((a) => a.acao !== "semMudanca") ?? [];

  return (
    <div className="preparar-ambiente" role="region" aria-label="Preparar o ambiente">
      <p>O ambiente do Claude ainda não está preparado para este projeto:</p>
      <ul>
        {ambiente.pendencias.map((p) => (
          <li key={p}>{p}</li>
        ))}
      </ul>
      {!plano && (
        <button className="btn" type="button" onClick={planejar} disabled={ocupado}>
          Ver o que será alterado
        </button>
      )}
      {plano && (
        <>
          <p>
            O app vai registrar o servidor MCP e os hooks na configuração do Claude Code. Nada é
            criado nas pastas do projeto, e o app faz uma cópia de segurança de cada arquivo antes
            de gravar.
          </p>
          <ul className="plano-preparo">
            {mudancas.map((a) => (
              <li key={`${a.arquivo}-${a.item.tipo === "hook" ? a.item.evento : a.item.chave}`}>
                {a.item.tipo === "hook"
                  ? `Hook ${a.item.evento}`
                  : `Servidor MCP "cba" para a pasta ${a.item.chave}`}{" "}
                <span className="muted">
                  ({a.acao === "atualizado" ? "atualizado" : "adicionado"} em {a.arquivo})
                </span>
              </li>
            ))}
            {proteger && (
              <li>
                Proteção contra edição: regra <code>deny</code> de <code>Edit</code> para as pastas
                do projeto <span className="muted">(adicionada nas configurações do usuário)</span>
              </li>
            )}
          </ul>
          {api.mudarProtecao && (
            <label className="protecao-opcional">
              <input
                type="checkbox"
                checked={proteger}
                onChange={(e) => setProteger(e.target.checked)}
              />{" "}
              Também ligar a proteção contra edição (opcional): o Claude fica impedido de editar
              arquivos e de usar o terminal (Bash) na pasta do projeto, mesmo que você aprove. Sem
              ela, o app só avisa quando a pasta for alterada.
            </label>
          )}
          <button className="btn" type="button" onClick={preparar} disabled={ocupado}>
            Concordo, preparar o ambiente
          </button>{" "}
          <button className="btn" type="button" onClick={() => setPlano(null)} disabled={ocupado}>
            Agora não
          </button>
        </>
      )}
      {falha && (
        <p className="error-text" role="alert">
          {falha}
        </p>
      )}
    </div>
  );
}

/**
 * Ambiente do Claude para um projeto: verifica e, se faltar algo, oferece o preparo
 * com consentimento. Avisa quem usa quando o ambiente fica pronto.
 */
export function AmbienteDoProjeto({
  api,
  projeto,
  aoMudar,
}: {
  api: ApiAmbiente;
  projeto: string;
  aoMudar?: (pronto: boolean) => void;
}) {
  const [ambiente, setAmbiente] = useState<AmbienteClaude | null>(null);
  const [falha, setFalha] = useState<string | null>(null);

  const verificar = useCallback(() => {
    setFalha(null);
    api
      .verificarAmbiente(projeto)
      .then((a) => {
        setAmbiente(a);
        aoMudar?.(a.pronto);
      })
      .catch((e: unknown) => setFalha(mensagemDe(e)));
  }, [api, projeto, aoMudar]);

  useEffect(verificar, [verificar]);

  return (
    <div className="ambiente-projeto">
      {!ambiente && !falha && <p className="muted">Verificando o ambiente do Claude…</p>}
      {falha && (
        <p className="error-text" role="alert">
          Não foi possível verificar o ambiente do Claude: {falha}
        </p>
      )}
      {ambiente?.pronto && (
        <p className="ok-text" role="status">
          O ambiente do Claude está preparado para este projeto.
        </p>
      )}
      {ambiente && !ambiente.pronto && (
        <PrepararAmbiente api={api} projeto={projeto} ambiente={ambiente} aoConcluir={verificar} />
      )}
    </div>
  );
}
