import { useCallback, useEffect, useState } from "react";
import { mensagemDe, type AmbienteClaude, type ApiAmbiente, type PlanoPreparo } from "./analise";

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
  api: ApiAmbiente;
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

/**
 * Ambiente do Claude para um sistema: verifica e, se faltar algo, oferece o preparo
 * com consentimento. Avisa quem usa quando o ambiente fica pronto.
 */
export function AmbienteDoSistema({
  api,
  sistema,
  aoMudar,
}: {
  api: ApiAmbiente;
  sistema: string;
  aoMudar?: (pronto: boolean) => void;
}) {
  const [ambiente, setAmbiente] = useState<AmbienteClaude | null>(null);
  const [falha, setFalha] = useState<string | null>(null);

  const verificar = useCallback(() => {
    setFalha(null);
    api
      .verificarAmbiente(sistema)
      .then((a) => {
        setAmbiente(a);
        aoMudar?.(a.pronto);
      })
      .catch((e: unknown) => setFalha(mensagemDe(e)));
  }, [api, sistema, aoMudar]);

  useEffect(verificar, [verificar]);

  return (
    <div className="ambiente-sistema">
      {!ambiente && !falha && <p className="suave">Verificando o ambiente do Claude…</p>}
      {falha && (
        <p className="trecho-erro" role="alert">
          Não foi possível verificar o ambiente do Claude: {falha}
        </p>
      )}
      {ambiente?.pronto && (
        <p className="confirmacao" role="status">
          O ambiente do Claude está preparado para este sistema.
        </p>
      )}
      {ambiente && !ambiente.pronto && (
        <PrepararAmbiente api={api} sistema={sistema} ambiente={ambiente} aoConcluir={verificar} />
      )}
    </div>
  );
}
