import { useState } from "react";
import { mensagemDe } from "./analise";
import type { EstadoClientes } from "./clientes";
import { Alerta, BarraAcoes, Secao, Selo } from "./componentes";
import { Passos, textoLogin, useEstadoClientes } from "./ClientesIA";
import type { ApiConfiguracoes } from "./configuracoes";
import { pendenciaDoCliente } from "./EscolhaCliente";
import type { ClientePreferido } from "./primeiroUso";

type Api = Pick<ApiConfiguracoes, "estadoClientes" | "escolherCliente" | "abrirLink">;

/** O que o app diz de um cliente já detectado: instalação e, no Claude Code, a conta. */
function chipsDoCliente(cliente: ClientePreferido, estado: EstadoClientes) {
  if (cliente === "terminalEmbutido") {
    const { instalacao, login } = estado.claudeCode;
    const instalado = instalacao.estado === "encontrado";
    const conta = textoLogin(estado);
    const chips = [
      {
        ok: instalado,
        texto: instalado
          ? `Instalado${instalacao.versao ? ` · versão ${instalacao.versao}` : ""}`
          : "Não instalado",
      },
    ];
    if (instalado && conta) {
      chips.push({
        ok: login.estado === "logado",
        texto: login.estado === "logado" ? "Conta conectada" : conta,
      });
    }
    return chips;
  }
  const instalado = estado.claudeDesktop.instalacao.estado === "encontrado";
  return [{ ok: instalado, texto: instalado ? "Instalado" : "Não instalado" }];
}

/**
 * Assistente de IA (#191): onde o analista conversa com o agente, escolhido na própria
 * tela de configurações (sem o passo do primeiro uso). A troca vale na hora e para as
 * próximas conversas; o estado do cliente escolhido (instalado, versão, conta) aparece
 * junto da opção, com “Verificar de novo” e o passo a passo do que falta.
 */
export function ConfigAssistente({
  api,
  cliente,
  aoMudar,
}: {
  api: Api;
  /** O cliente gravado; `null` vale o padrão (terminal do app). */
  cliente: ClientePreferido | null;
  aoMudar: () => void;
}) {
  const [escolhido, setEscolhido] = useState<ClientePreferido>(cliente ?? "terminalEmbutido");
  const [gravando, setGravando] = useState(false);
  const [falha, setFalha] = useState<string | null>(null);
  const [salvo, setSalvo] = useState(false);
  const { carga, verificar } = useEstadoClientes(api.estadoClientes);
  const estado = carga.tipo === "pronto" ? carga.estado : null;
  const desktopIndisponivel = estado?.claudeDesktop.instalacao.estado === "naoSuportado";
  const pendencia = estado ? pendenciaDoCliente(escolhido, estado, true) : null;

  const escolher = (novo: ClientePreferido) => {
    if (novo === escolhido) return;
    setGravando(true);
    setFalha(null);
    setSalvo(false);
    api
      .escolherCliente(novo)
      .then(() => {
        setEscolhido(novo);
        setSalvo(true);
        aoMudar();
      })
      .catch((e: unknown) => setFalha(mensagemDe(e)))
      .finally(() => setGravando(false));
  };

  const orientacao =
    estado && pendencia
      ? escolhido === "terminalEmbutido"
        ? estado.claudeCode.orientacao
        : estado.claudeDesktop.orientacao
      : [];

  const chips = (c: ClientePreferido) =>
    estado && escolhido === c ? (
      <span className="config-linha-chips">
        {chipsDoCliente(c, estado).map((x) => (
          <Selo key={x.texto} tom={x.ok ? "concluido" : "atencao"}>
            {x.ok ? "✓ " : ""}
            {x.texto}
          </Selo>
        ))}
      </span>
    ) : null;

  return (
    <Secao titulo="Assistente de IA">
      <p className="muted">Onde você conversa com o agente durante as análises.</p>
      <fieldset className="cartoes-opcao">
        <legend className="sr-only">Onde conversar</legend>
        <label className="cartao-opcao" data-escolhido={escolhido === "terminalEmbutido"}>
          <input
            type="radio"
            name="cliente"
            value="terminalEmbutido"
            checked={escolhido === "terminalEmbutido"}
            disabled={gravando}
            onChange={() => escolher("terminalEmbutido")}
          />
          <span className="cartao-opcao-corpo">
            <span className="cartao-opcao-titulo">
              <strong>No terminal do app</strong>
              <Selo>Claude Code · recomendado</Selo>
              {!cliente && <span className="muted">(padrão)</span>}
            </span>
            <span className="muted">A conversa fica ao lado do documento, numa janela só.</span>
            {chips("terminalEmbutido")}
          </span>
        </label>
        <label
          className="cartao-opcao"
          data-escolhido={escolhido === "claudeDesktop"}
          data-indisponivel={desktopIndisponivel}
        >
          <input
            type="radio"
            name="cliente"
            value="claudeDesktop"
            checked={escolhido === "claudeDesktop"}
            disabled={gravando || desktopIndisponivel}
            onChange={() => escolher("claudeDesktop")}
          />
          <span className="cartao-opcao-corpo">
            <span className="cartao-opcao-titulo">
              <strong>No Claude Desktop</strong>
            </span>
            <span className="muted">
              Para quem não quer usar terminal. O app abre a conversa no Claude Desktop.
            </span>
            {desktopIndisponivel && (
              <span className="attention-text">
                Indisponível neste computador: o Claude Desktop só existe para macOS e Windows.
              </span>
            )}
            {chips("claudeDesktop")}
          </span>
        </label>
      </fieldset>

      {carga.tipo === "verificando" && <p className="muted">Verificando o cliente…</p>}
      {carga.tipo === "erro" && (
        <Alerta tom="erro" titulo="Não foi possível verificar os clientes de IA.">
          A escolha vale mesmo assim; confira depois.
        </Alerta>
      )}
      {pendencia && (
        <Alerta tom="atencao" titulo={pendencia}>
          <Passos passos={orientacao} abrirLink={api.abrirLink} />
        </Alerta>
      )}
      {salvo && (
        <p className="ok-text" role="status">
          Salvo. A troca vale para as próximas conversas; as que estão abertas continuam onde estão.
        </p>
      )}
      {falha && (
        <Alerta tom="erro" titulo={falha} />
      )}
      <BarraAcoes
        rotulo="Ações do assistente de IA"
        secundarias={[
          { texto: "Verificar de novo", aoEscolher: verificar, desabilitado: carga.tipo === "verificando" },
        ]}
      />
    </Secao>
  );
}
