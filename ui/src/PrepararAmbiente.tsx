import { useCallback, useEffect, useState } from "react";
import {
  mensagemDe,
  type AlteracaoPreparo,
  type AmbienteClaude,
  type ApiAmbiente,
  type PlanoPreparo,
} from "./analise";

/**
 * Preparo do ambiente com consentimento: mostra o que falta, o que será alterado na
 * configuração do Claude (sem gravar) e só prepara quando o analista concorda.
 */
export function PrepararAmbiente({
  api,
  projeto,
  ambiente,
  aoConcluir,
  oferecerProtecao = true,
}: {
  api: ApiAmbiente;
  projeto: string;
  ambiente: AmbienteClaude;
  aoConcluir: () => void;
  /** Falso onde a proteção tem lugar próprio (a aba Agente das configurações, #192). */
  oferecerProtecao?: boolean;
}) {
  const [plano, setPlano] = useState<PlanoPreparo | null>(null);
  const [ocupado, setOcupado] = useState(false);
  const [falha, setFalha] = useState<string | null>(null);
  // Proteção opcional contra edição (#29): desligada por padrão.
  const [proteger, setProteger] = useState(false);
  // Concordância com a troca dos avisos de outra instalação do app (#213).
  const [aceitaTroca, setAceitaTroca] = useState(false);

  const planejar = () => {
    setOcupado(true);
    setFalha(null);
    api
      .planejarPreparo(projeto)
      .then((p) => {
        setAceitaTroca(false);
        setPlano(p);
      })
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
  const outras = outrasInstalacoes(mudancas);

  return (
    <div className="preparar-ambiente" role="region" aria-label="Ligar o agente a este projeto">
      <p>O agente ainda não está ligado a este projeto.</p>
      <details className="detalhes-tecnicos">
        <summary>Detalhes</summary>
        <ul>
          {ambiente.pendencias.map((p) => (
            <li key={p}>{p}</li>
          ))}
        </ul>
      </details>
      {!plano && (
        <button className="btn" type="button" onClick={planejar} disabled={ocupado}>
          Ver e preparar
        </button>
      )}
      {plano && (
        <>
          <p>
            O app vai ligar o agente a este projeto e, para isso, vai alterar a configuração do
            Claude (detalhes abaixo). Antes de gravar, faz uma cópia de segurança de cada arquivo e,
            ao desfazer, tudo isso sai.
          </p>
          <details className="detalhes-tecnicos">
            <summary>Detalhes</summary>
          <p>
            O app vai registrar o servidor MCP na configuração do Claude Code, marcar a pasta do
            projeto como confiável (para o Claude não perguntar isso na primeira conversa) e gravar
            os hooks no arquivo <code>.claude/settings.local.json</code> da pasta, que vale só para
            as conversas nela e fica fora do git. É o único arquivo criado na pasta do projeto. O
            app faz uma cópia de segurança de cada arquivo antes de gravar e, ao desfazer o
            preparo, tudo isso sai.
          </p>
          <ul className="plano-preparo">
            {porArquivo(mudancas).map(([arquivo, itens]) => (
              <li key={arquivo}>
                <code>{arquivo}</code>
                <ul>
                  {itens.map((a) => (
                    <li key={`${a.item.tipo}-${chaveDoItem(a.item)}`}>
                      {descreverItem(a.item)}{" "}
                      <span className="muted">
                        ({a.acao === "atualizado" ? "atualizado" : "adicionado"}
                        {a.outraInstalacao ? "; hoje vai para outra instalação do app" : ""})
                      </span>
                    </li>
                  ))}
                </ul>
              </li>
            ))}
            {proteger && (
              <li>
                Proteção contra edição: regra <code>deny</code> de <code>Edit</code> para as pastas
                do projeto{" "}
                <span className="muted">
                  (no <code>.claude/settings.local.json</code> de cada pasta)
                </span>
              </li>
            )}
          </ul>
          </details>
          {outras.length > 0 && (
            <div className="aviso-outra-instalacao attention-text" role="note">
              <p>
                <strong>Hoje os avisos do Claude vão para outra instalação do app</strong>, em{" "}
                {outras.map((o, i) => (
                  <span key={o}>
                    {i > 0 ? ", " : ""}
                    <code>{o}</code>
                  </span>
                ))}
                . Ao preparar, eles passam a vir para esta, e a outra deixa de recebê-los. Ao
                desfazer o preparo, o app devolve os avisos para a outra instalação.
              </p>
              <label>
                <input
                  type="checkbox"
                  checked={aceitaTroca}
                  onChange={(e) => setAceitaTroca(e.target.checked)}
                />{" "}
                Entendo e quero que os avisos venham para esta instalação.
              </label>
            </div>
          )}
          {api.mudarProtecao && oferecerProtecao && (
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
          <div className="acoes">
            <button className="btn" type="button" onClick={() => setPlano(null)} disabled={ocupado}>
              Agora não
            </button>
            <button
              className="btn btn-primary"
              type="button"
              onClick={preparar}
              disabled={ocupado || (outras.length > 0 && !aceitaTroca)}
            >
              Concordo, ligar o agente
            </button>
          </div>
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

function chaveDoItem(item: AlteracaoPreparo["item"]): string {
  switch (item.tipo) {
    case "hook":
      return item.evento;
    case "exclusaoGit":
      return item.raiz;
    default:
      return item.chave;
  }
}

/** Alterações agrupadas pelo arquivo, na ordem em que aparecem. */
export function porArquivo(alteracoes: AlteracaoPreparo[]): [string, AlteracaoPreparo[]][] {
  const grupos: [string, AlteracaoPreparo[]][] = [];
  for (const a of alteracoes) {
    const grupo = grupos.find(([arquivo]) => arquivo === a.arquivo);
    if (grupo) grupo[1].push(a);
    else grupos.push([a.arquivo, [a]]);
  }
  return grupos;
}

/** Texto de um item do plano. */
export function descreverItem(item: AlteracaoPreparo["item"]): string {
  switch (item.tipo) {
    case "hook":
      return `Hook ${item.evento}`;
    case "servidorMcp":
      return `Servidor MCP "cba" para a pasta ${item.chave}`;
    case "confianca":
      return `Pasta ${item.chave} marcada como confiável no Claude Code (sem a pergunta "Quick safety check", em inglês, na primeira conversa)`;
    case "exclusaoGit":
      return `Arquivo .claude/settings.local.json fora do git em ${item.raiz} (linha no info/exclude, que não é versionado)`;
  }
}

/** Executáveis de outras instalações do app que o preparo tira do lugar, sem repetição. */
export function outrasInstalacoes(alteracoes: AlteracaoPreparo[]): string[] {
  const v: string[] = [];
  for (const a of alteracoes) {
    // O mesmo executável vem com "\\" no servidor MCP e com "/" nos hooks (Windows).
    const normal = (c: string) => c.replaceAll("\\", "/");
    const o = a.outraInstalacao;
    if (a.acao === "atualizado" && o && !v.some((x) => normal(x) === normal(o))) {
      v.push(o);
    }
  }
  return v;
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
