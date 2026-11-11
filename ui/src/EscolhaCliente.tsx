import { useState } from "react";
import { mensagemDe } from "./analise";
import type { EstadoClientes } from "./clientes";
import { CartaoClaudeCode, CartaoClaudeDesktop, useEstadoClientes } from "./ClientesIA";
import type { ApiPrimeiroUso, ClientePreferido } from "./primeiroUso";

type Api = Pick<ApiPrimeiroUso, "estadoClientes" | "escolherCliente" | "abrirLink">;

/** O cliente escolhido está pronto para seguir? Devolve o que falta, ou `null`. */
export function pendenciaDoCliente(
  cliente: ClientePreferido,
  estado: EstadoClientes,
  desktopConfirmado: boolean,
): string | null {
  if (cliente === "terminalEmbutido") {
    const { instalacao, login } = estado.claudeCode;
    if (instalacao.estado !== "encontrado") {
      return "Instale o Claude Code seguindo o passo a passo e clique em “Verificar de novo”.";
    }
    if (login.estado === "naoLogado" || login.estado === "naoVerificado") {
      return "Entre no Claude Code com a sua conta (passo a passo acima) e clique em “Verificar de novo”.";
    }
    return null;
  }
  const { instalacao } = estado.claudeDesktop;
  if (instalacao.estado === "naoSuportado") {
    return "O Claude Desktop não está disponível neste sistema. Escolha o terminal embutido.";
  }
  if (instalacao.estado !== "encontrado") {
    return "Instale o Claude Desktop pela página oficial e clique em “Verificar de novo”.";
  }
  if (!desktopConfirmado) {
    return "Abra o Claude Desktop, entre com a sua conta e confirme abaixo.";
  }
  return null;
}

/**
 * Passo 1 do primeiro uso (e item das configurações): como conversar com a IA, com a
 * verificação do cliente escolhido e o passo a passo oficial do que falta. O app não
 * faz login por ninguém: o analista entra pelo próprio cliente.
 */
export function EscolhaCliente({
  api,
  inicial,
  aoConcluir,
  rotuloContinuar = "Continuar",
}: {
  api: Api;
  inicial?: ClientePreferido | null;
  aoConcluir: (cliente: ClientePreferido) => void;
  rotuloContinuar?: string;
}) {
  const [cliente, setCliente] = useState<ClientePreferido>(inicial ?? "terminalEmbutido");
  const [desktopConfirmado, setDesktopConfirmado] = useState(false);
  const [gravando, setGravando] = useState(false);
  const [falha, setFalha] = useState<string | null>(null);
  const { carga, verificar } = useEstadoClientes(api.estadoClientes);

  const estado = carga.tipo === "pronto" ? carga.estado : null;
  const desktopIndisponivel = estado?.claudeDesktop.instalacao.estado === "naoSuportado";
  const pendencia = estado ? pendenciaDoCliente(cliente, estado, desktopConfirmado) : null;
  const loginIncerto =
    cliente === "terminalEmbutido" && estado?.claudeCode.login.estado === "indeterminado";

  const continuar = () => {
    setGravando(true);
    setFalha(null);
    api
      .escolherCliente(cliente)
      .then(() => aoConcluir(cliente))
      .catch((e: unknown) => setFalha(mensagemDe(e)))
      .finally(() => setGravando(false));
  };

  return (
    <section className="passo-assistente" aria-label="Cliente de IA">
      <h2>Como você prefere conversar com a IA?</h2>
      <fieldset className="opcoes-cliente">
        <legend className="suave">Você pode trocar depois, nas configurações.</legend>
        <label className={cliente === "terminalEmbutido" ? "opcao escolhida" : "opcao"}>
          <input
            type="radio"
            name="cliente"
            value="terminalEmbutido"
            checked={cliente === "terminalEmbutido"}
            onChange={() => setCliente("terminalEmbutido")}
          />
          <strong>Claude Code no terminal do app (recomendado)</strong>
          <span className="suave">Tudo numa janela só: a conversa ao lado do documento.</span>
        </label>
        <label className={cliente === "claudeDesktop" ? "opcao escolhida" : "opcao"}>
          <input
            type="radio"
            name="cliente"
            value="claudeDesktop"
            checked={cliente === "claudeDesktop"}
            disabled={desktopIndisponivel}
            onChange={() => setCliente("claudeDesktop")}
          />
          <strong>Claude Desktop</strong>
          <span className="suave">
            {desktopIndisponivel
              ? "Não disponível neste sistema (só macOS e Windows)."
              : "Para quem não quer usar terminal: a conversa no Desktop, o documento aqui."}
          </span>
        </label>
      </fieldset>

      {carga.tipo === "verificando" && <p className="suave">Verificando o cliente…</p>}
      {carga.tipo === "erro" && (
        <p className="trecho-erro" role="alert">
          Não foi possível verificar os clientes de IA. Você pode seguir assim mesmo e conferir
          depois.
        </p>
      )}
      {estado && cliente === "terminalEmbutido" && (
        <CartaoClaudeCode estado={estado} abrirLink={api.abrirLink} titulo="Claude Code" />
      )}
      {estado && cliente === "claudeDesktop" && (
        <>
          <CartaoClaudeDesktop estado={estado} abrirLink={api.abrirLink} />
          {estado.claudeDesktop.instalacao.estado === "encontrado" && (
            <label>
              <input
                type="checkbox"
                checked={desktopConfirmado}
                onChange={(e) => setDesktopConfirmado(e.target.checked)}
              />{" "}
              Já entrei no Claude Desktop com a minha conta
            </label>
          )}
        </>
      )}
      {loginIncerto && (
        <p className="suave">
          Não deu para confirmar o login. Se o Claude Code pedir para entrar quando abrir, entre com
          a sua conta.
        </p>
      )}
      {pendencia && (
        <p className="pendencia" role="status">
          {pendencia}
        </p>
      )}
      {falha && (
        <p className="trecho-erro" role="alert">
          {falha}
        </p>
      )}
      <div className="acoes">
        <button type="button" onClick={verificar} disabled={carga.tipo === "verificando"}>
          Verificar de novo
        </button>
        <button
          type="button"
          className="primario"
          onClick={continuar}
          disabled={
            gravando ||
            carga.tipo === "verificando" ||
            (carga.tipo === "pronto" && pendencia !== null)
          }
        >
          {rotuloContinuar}
        </button>
      </div>
    </section>
  );
}
