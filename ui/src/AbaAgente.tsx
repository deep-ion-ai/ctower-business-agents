import { useEffect, useState } from "react";
import { mensagemDe, type SituacaoProtecao } from "./analise";
import { AbrirTexto } from "./AbrirTexto";
import type { PropsConfigProjeto } from "./ConfigProjeto";
import { ESTADO_DO_TEXTO, type AmbienteDetalhado } from "./configuracoes";
import { PrepararAmbiente } from "./PrepararAmbiente";

const SITUACAO = {
  ok: "registrado",
  ausente: "ausente",
  desatualizado: "desatualizado",
  conflito: "conflito (há um servidor “cba” que não é do app)",
} as const;

/** Linhas da prévia das instruções do projeto. */
const LINHAS_DA_PREVIA = 6;

/**
 * Aba “Agente” (#192): o preparo em linguagem de negócio, com os detalhes técnicos
 * recolhidos; a proteção contra edição como interruptor, com as regras confirmadas
 * antes; e as instruções do projeto com prévia, Editar e abrir no computador.
 */
export function AbaAgente(props: PropsConfigProjeto) {
  const { projeto, textos } = props;
  const regras = textos.find((t) => t.grupo === "regrasDoProjeto" && t.id === projeto.id);
  return (
    <div className="aba-agente">
      <PreparoDoAgente {...props} />
      <Protecao {...props} />
      {regras && <InstrucoesDoProjeto {...props} arquivo={regras.arquivo} />}
    </div>
  );
}

function itensDoPreparo(a: AmbienteDetalhado) {
  const v = a.verificacao;
  const mcp = v ? v.servidorMcp === "ok" : a.pronto;
  const hooks = v ? !v.hooksDesativados && v.hooks.every((h) => h.situacao === "ok") : a.pronto;
  const confiavel = v ? v.pastaConfiavel : a.pronto;
  return [
    {
      ok: mcp,
      titulo: mcp ? "Ferramentas do app conectadas ao Claude" : "Ferramentas do app não conectadas",
      texto: mcp
        ? "É por elas que o agente escreve no documento da análise."
        : "O agente não consegue registrar afirmações no documento.",
    },
    {
      ok: hooks,
      titulo: hooks ? "Avisos automáticos ligados" : "Avisos automáticos desligados",
      texto: hooks
        ? "O app fica sabendo quando o agente termina ou pede algo."
        : v?.hooksDesativados
          ? "Os hooks estão desativados nas configurações do Claude."
          : "O app não sabe quando o agente termina.",
    },
    {
      ok: confiavel,
      titulo: confiavel ? "Pasta confiável para o Claude" : "Pasta ainda não confiável para o Claude",
      texto: confiavel
        ? "A primeira conversa abre direto, sem perguntas."
        : "Na primeira conversa, o Claude pergunta em inglês se você confia na pasta.",
    },
  ];
}

function PreparoDoAgente({ api, cfg, projeto, estado, aoReverificarAmbiente }: PropsConfigProjeto) {
  const ambiente = estado.ambiente;
  const v = ambiente?.verificacao;
  return (
    <section className="cartao-config" aria-label="Preparo do agente">
      <div className="cartao-config-topo">
        <h4>O agente neste projeto</h4>
        <span
          className="badge"
          data-tone={!ambiente ? undefined : ambiente.pronto ? "done" : "attention"}
        >
          {ambiente === undefined
            ? "verificando…"
            : ambiente === null
              ? "não verificado"
              : ambiente.pronto
                ? "✓ preparado"
                : "não preparado"}
        </span>
      </div>
      <div className="linha-config preparo-corpo">
        {ambiente === null && (
          <p className="error-text" role="alert">
            Não foi possível verificar a configuração do Claude deste projeto.{" "}
            <button
              type="button"
              className="link"
              onClick={() => void aoReverificarAmbiente(projeto.id)}
            >
              Verificar de novo
            </button>
          </p>
        )}
        {ambiente && (
          <ul className="itens-preparo">
            {itensDoPreparo(ambiente).map((i) => (
              <li key={i.titulo} data-estado={i.ok ? "ok" : "problema"}>
                <span className="saude-icone" aria-hidden="true">
                  {i.ok ? "✓" : "!"}
                </span>
                <div className="saude-texto">
                  <span>{i.titulo}</span>
                  <span className="muted">{i.texto}</span>
                </div>
              </li>
            ))}
          </ul>
        )}
        {ambiente && !ambiente.pronto && (
          <PrepararAmbiente
            api={api}
            projeto={projeto.id}
            ambiente={ambiente}
            oferecerProtecao={false}
            aoConcluir={() => void aoReverificarAmbiente(projeto.id)}
          />
        )}
        {ambiente && (
          <details className="detalhes-tecnicos">
            <summary>Detalhes técnicos</summary>
            <ul className="detalhe-ambiente" aria-label="Situação do ambiente">
              {v && <li>Servidor MCP “cba”: {SITUACAO[v.servidorMcp]}</li>}
              {v && (
                <li>
                  Hooks: {v.hooks.filter((h) => h.situacao === "ok").length} de {v.hooks.length}{" "}
                  registrados
                  {v.hooksDesativados && " (desativados nas configurações do Claude)"}
                </li>
              )}
              {v && <li>Confiança na pasta: {v.pastaConfiavel ? "marcada" : "ausente"}</li>}
              {v && (
                <li>
                  Pasta: <span className="caminho">{v.diretorio}</span>
                </li>
              )}
              {cfg.configClaude && (
                <li>
                  Arquivos: <span className="caminho">{cfg.configClaude.estado}</span> (servidor
                  MCP) e <span className="caminho">{cfg.configClaude.settings}</span> (hooks e
                  proteção)
                </li>
              )}
              {ambiente.pendencias.map((p) => (
                <li key={p}>{p}</li>
              ))}
            </ul>
          </details>
        )}
      </div>
    </section>
  );
}

function Protecao({ api, projeto }: PropsConfigProjeto) {
  const [situacao, setSituacao] = useState<SituacaoProtecao | null>(null);
  const [confirmando, setConfirmando] = useState(false);
  const [ocupado, setOcupado] = useState(false);
  const [falha, setFalha] = useState<string | null>(null);

  useEffect(() => {
    let ativo = true;
    api
      .situacaoProtecao(projeto.id)
      .then((s) => ativo && setSituacao(s))
      .catch((e: unknown) => ativo && setFalha(mensagemDe(e)));
    return () => {
      ativo = false;
    };
  }, [api, projeto.id]);

  const mudar = (ligada: boolean) => {
    setOcupado(true);
    setFalha(null);
    api
      .mudarProtecao(projeto.id, ligada)
      .then((s) => {
        setSituacao(s);
        setConfirmando(false);
      })
      .catch((e: unknown) => setFalha(mensagemDe(e)))
      .finally(() => setOcupado(false));
  };

  const ligada = situacao?.ligada ?? false;
  return (
    <section className="cartao-config" aria-label="Proteção contra edição">
      <div className="linha-config protecao-corpo">
        <div className="protecao-texto">
          <h4 id={`protecao-${projeto.id}`}>Impedir que o agente altere arquivos</h4>
          {situacao && (
            <p className="muted">
              {situacao.ligada
                ? situacao.completa
                  ? "Ligada: o Claude não edita arquivos nem usa o terminal nas pastas do projeto."
                  : "Ligada, mas faltam regras no arquivo de configuração do Claude. Ligue de novo para completar."
                : "Desligada (padrão): o agente pode editar arquivos e usar o terminal; o app avisa quando a pasta do projeto é alterada."}
            </p>
          )}
          {situacao && confirmando && (
            <div className="confirmar-alteracao" role="region" aria-label="Confirmar a proteção">
              {situacao.ligada ? (
                <p>
                  O app vai tirar do <code>.claude/settings.local.json</code> das pastas só as regras
                  que ele mesmo acrescentou, e o terminal (Bash) volta a funcionar nas conversas
                  deste projeto.
                </p>
              ) : (
                <>
                  <p>
                    O app vai acrescentar estas regras no <code>.claude/settings.local.json</code>{" "}
                    das pastas (pessoal e fora do git), que valem mesmo que você aprove a edição, e
                    negar o terminal ({situacao.negadasPeloHook.join(", ")}) nas conversas deste
                    projeto:
                  </p>
                  <ul>
                    {situacao.regras.map((r) => (
                      <li key={r.regra}>
                        <code>{r.regra}</code>
                        {r.arquivo && <span className="muted"> em {r.arquivo}</span>}
                      </li>
                    ))}
                  </ul>
                </>
              )}
              <div className="acoes">
                <button
                  className="btn"
                  type="button"
                  onClick={() => setConfirmando(false)}
                  disabled={ocupado}
                >
                  Cancelar
                </button>
                <button
                  type="button"
                  className="btn btn-primary"
                  onClick={() => mudar(!situacao.ligada)}
                  disabled={ocupado}
                >
                  {situacao.ligada ? "Desligar a proteção" : "Ligar a proteção"}
                </button>
              </div>
            </div>
          )}
        </div>
        <button
          type="button"
          role="switch"
          className="interruptor"
          aria-checked={ligada}
          aria-labelledby={`protecao-${projeto.id}`}
          disabled={!situacao || confirmando || ocupado}
          onClick={() => setConfirmando(true)}
        >
          <span aria-hidden="true" />
        </button>
      </div>
      {falha && (
        <p className="error-text linha-config" role="alert">
          {falha}
        </p>
      )}
    </section>
  );
}

function InstrucoesDoProjeto({
  api,
  textos,
  projeto,
  aoAbrirTexto,
  aoEditar,
  arquivo,
}: PropsConfigProjeto & { arquivo: string }) {
  const regras = textos.find((t) => t.arquivo === arquivo)!;
  const [texto, setTexto] = useState<string | null>(null);
  useEffect(() => {
    let ativo = true;
    api
      .lerTexto(arquivo)
      .then((l) => ativo && setTexto(l.texto))
      .catch(() => ativo && setTexto(null));
    return () => {
      ativo = false;
    };
  }, [api, arquivo, regras.estado]);

  const linhas = (texto ?? "").split("\n").filter((l) => l.trim() !== "");
  const resto = linhas.length - LINHAS_DA_PREVIA;
  return (
    <section className="cartao-config" aria-label={`Instruções do projeto ${projeto.nome}`}>
      <div className="cartao-config-topo">
        <h4>Instruções para o agente</h4>
        <span className="badge">{ESTADO_DO_TEXTO[regras.estado]}</span>
      </div>
      <div className="linha-config instrucoes-corpo">
        <p className="muted">
          Valem em toda análise deste projeto, além das regras gerais ({regras.arquivo}).
        </p>
        {texto !== null && (
          <pre className="previa-instrucoes" aria-label="Prévia das instruções">
            {linhas.slice(0, LINHAS_DA_PREVIA).join("\n")}
            {resto > 0 && `\n… mais ${resto} ${resto === 1 ? "linha" : "linhas"}`}
          </pre>
        )}
        <div className="acoes">
          <AbrirTexto api={api} texto={regras} aoAbrir={aoAbrirTexto} aoEditar={aoEditar} />
        </div>
      </div>
    </section>
  );
}
