import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { TelaConfiguracoes } from "./TelaConfiguracoes";
import {
  BASE,
  apiConfiguracoesFalsa,
  configuracoesFicticias,
} from "./test/configuracoesFalso";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(), Channel: vi.fn() }));

function montar(api = apiConfiguracoesFalsa()) {
  const aoMudarSistemas = vi.fn();
  render(<TelaConfiguracoes api={api} aoMudarSistemas={aoMudarSistemas} />);
  return { api, aoMudarSistemas };
}

async function cartao() {
  return within(await screen.findByRole("article", { name: "Faturamento Exemplo" }));
}

describe("TelaConfiguracoes", () => {
  it("mostra o cliente, a base, os sistemas e os textos editáveis", async () => {
    montar();
    const cliente = within(await screen.findByRole("region", { name: "Cliente de IA" }));
    expect(cliente.getByText("Claude Desktop")).toBeInTheDocument();
    expect(
      within(screen.getByRole("region", { name: "Pasta da base" })).getByText(BASE),
    ).toBeInTheDocument();
    const sistema = await cartao();
    expect(sistema.getByText("/codigo/faturamento-exemplo")).toBeInTheDocument();
    expect(await sistema.findByText("Servidor MCP “cba”: registrado")).toBeInTheDocument();
    expect(sistema.getByText("Hooks: 2 de 2 registrados")).toBeInTheDocument();
    expect(await sistema.findByText(/Desligada \(padrão\)/)).toBeInTheDocument();

    const textos = within(screen.getByRole("region", { name: "Textos editáveis" }));
    const templates = within(textos.getByRole("group", { name: "Templates dos documentos" }));
    expect(templates.getByText("editado na base (sobrepõe o padrão)")).toBeInTheDocument();
    const roteiros = within(textos.getByRole("group", { name: "Roteiros dos fluxos" }));
    expect(roteiros.getByText("padrão do app")).toBeInTheDocument();
    expect(roteiros.getByRole("button", { name: /Perguntar ao sistema/ })).toHaveTextContent(
      "Criar a cópia e abrir",
    );
  });

  it("troca o cliente preferido com o passo do primeiro uso", async () => {
    const { api } = montar();
    fireEvent.click(await screen.findByRole("button", { name: "Trocar o cliente" }));
    fireEvent.click(await screen.findByRole("radio", { name: /terminal do app/ }));
    const salvar = screen.getByRole("button", { name: "Salvar" });
    await waitFor(() => expect(salvar).toBeEnabled());
    fireEvent.click(salvar);
    await waitFor(() => expect(api.escolherCliente).toHaveBeenCalledWith("terminalEmbutido"));
    await waitFor(() => expect(api.configuracoes).toHaveBeenCalledTimes(2));
  });

  it("move a base só depois da confirmação", async () => {
    const { api, aoMudarSistemas } = montar();
    fireEvent.click(await screen.findByRole("button", { name: "Trocar a pasta da base" }));
    fireEvent.change(screen.getByLabelText(/Pasta da base \(caminho completo\)/), {
      target: { value: "/Users/analista/OneDrive/base" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Continuar" }));
    const confirmar = within(
      await screen.findByRole("region", { name: "Confirmar a troca da base" }),
    );
    expect(await confirmar.findByText(/tem 12 arquivos/)).toBeInTheDocument();
    expect(confirmar.getByRole("radio", { name: /Mover os documentos/ })).toBeChecked();
    const mover = confirmar.getByRole("button", { name: "Mover a base" });
    expect(mover).toBeDisabled();
    fireEvent.click(confirmar.getByRole("checkbox", { name: /Confirmo: mover/ }));
    fireEvent.click(mover);
    await waitFor(() =>
      expect(api.trocarBase).toHaveBeenCalledWith("/Users/analista/OneDrive/base", "mover"),
    );
    expect(await screen.findByText(/12 arquivos copiados e conferidos/)).toBeInTheDocument();
    expect(screen.getByText(/Índice reconstruído com 5 documentos/)).toBeInTheDocument();
    expect(aoMudarSistemas).toHaveBeenCalled();
  });

  it("só reaponta quando a pasta nova já tem arquivos", async () => {
    const api = apiConfiguracoesFalsa();
    api.avaliarTroca.mockImplementation((destino: string) =>
      Promise.resolve({
        destino: { caminho: destino, estado: "base" as const, recusa: null, git: true },
        atual: BASE,
        arquivos: 12,
        bytes: 1,
        recusaMover: "para mover, escolha uma pasta nova ou vazia",
        recusaReapontar: null,
        sessoesAbertas: 1,
      }),
    );
    montar(api);
    fireEvent.click(await screen.findByRole("button", { name: "Trocar a pasta da base" }));
    fireEvent.change(screen.getByLabelText(/Pasta da base \(caminho completo\)/), {
      target: { value: "/outra/base" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Continuar" }));
    const confirmar = within(
      await screen.findByRole("region", { name: "Confirmar a troca da base" }),
    );
    await waitFor(() =>
      expect(confirmar.getByRole("radio", { name: /Só passar a usar/ })).toBeChecked(),
    );
    expect(confirmar.getByRole("radio", { name: /Mover os documentos/ })).toBeDisabled();
    expect(confirmar.getByText(/escolha uma pasta nova ou vazia/)).toBeInTheDocument();
    expect(confirmar.getByText(/Feche as conversas/)).toBeInTheDocument();
    fireEvent.click(confirmar.getByRole("checkbox", { name: /Confirmo: passar a usar/ }));
    fireEvent.click(confirmar.getByRole("button", { name: "Usar a pasta nova" }));
    await waitFor(() => expect(api.trocarBase).toHaveBeenCalledWith("/outra/base", "reapontar"));
  });

  it("a escolha da pasta mostra a recusa da base", async () => {
    const api = apiConfiguracoesFalsa();
    api.avaliarBase.mockResolvedValue({
      caminho: "/tmp/base",
      estado: "nova",
      recusa: "a base não pode ficar numa pasta temporária",
      git: false,
    });
    montar(api);
    fireEvent.click(await screen.findByRole("button", { name: "Trocar a pasta da base" }));
    fireEvent.change(screen.getByLabelText(/Pasta da base \(caminho completo\)/), {
      target: { value: "/tmp/base" },
    });
    expect(await screen.findByText(/pasta temporária/, {}, { timeout: 2000 })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Continuar" })).toBeDisabled();
    expect(api.avaliarTroca).not.toHaveBeenCalled();
  });

  it("ajusta as exclusões com a prévia do inventário", async () => {
    const { api, aoMudarSistemas } = montar();
    const sistema = await cartao();
    fireEvent.click(sistema.getByText("Exclusões: o que fica fora da análise"));
    const campo = sistema.getByLabelText(/Excluir também/);
    expect(campo).toHaveValue("docs/antigos/");
    fireEvent.change(campo, { target: { value: "docs/antigos/\ncobol/" } });
    fireEvent.click(sistema.getByRole("button", { name: "Ver o que será analisado" }));
    await waitFor(() =>
      expect(api.previaExclusoes).toHaveBeenCalledWith({
        sistema: "faturamento-exemplo",
        exclusoes: ["docs/antigos/", "cobol/"],
        inclusoes: [],
      }),
    );
    // Reincluir um item ignorado pela prévia.
    fireEvent.click(await sistema.findByRole("button", { name: "Incluir .env na análise" }));
    await waitFor(() =>
      expect(api.previaExclusoes).toHaveBeenLastCalledWith(
        expect.objectContaining({ inclusoes: ["/.env"] }),
      ),
    );
    fireEvent.click(sistema.getByRole("button", { name: "Salvar as exclusões" }));
    await waitFor(() =>
      expect(api.salvarExclusoes).toHaveBeenCalledWith({
        sistema: "faturamento-exemplo",
        exclusoes: ["docs/antigos/", "cobol/"],
        inclusoes: ["/.env"],
      }),
    );
    expect(await sistema.findByText(/Exclusões salvas/)).toBeInTheDocument();
    expect(aoMudarSistemas).toHaveBeenCalled();
  });

  it("desfaz o ambiente com confirmação e verifica de novo", async () => {
    const { api } = montar();
    const sistema = await cartao();
    await waitFor(() => expect(api.verificarAmbiente).toHaveBeenCalledTimes(2));
    fireEvent.click(sistema.getByRole("button", { name: "Desfazer o ambiente" }));
    const confirmar = within(
      sistema.getByRole("region", { name: "Confirmar desfazer o ambiente" }),
    );
    fireEvent.click(confirmar.getByRole("button", { name: "Desfazer" }));
    await waitFor(() => expect(api.desfazerAmbiente).toHaveBeenCalledWith("faturamento-exemplo"));
    expect(await sistema.findByText(/Ambiente desfeito/)).toBeInTheDocument();
    await waitFor(() => expect(api.verificarAmbiente).toHaveBeenCalledTimes(4));
    expect(api.situacaoProtecao).toHaveBeenCalledTimes(2);
  });

  it("liga a proteção mostrando as regras antes", async () => {
    const { api } = montar();
    const sistema = await cartao();
    fireEvent.click(await sistema.findByRole("button", { name: "Ligar a proteção" }));
    expect(sistema.getByText("Edit(//codigo/faturamento-exemplo/**)")).toBeInTheDocument();
    expect(api.mudarProtecao).not.toHaveBeenCalled();
    fireEvent.click(sistema.getByRole("button", { name: "Concordo, ligar" }));
    await waitFor(() =>
      expect(api.mudarProtecao).toHaveBeenCalledWith("faturamento-exemplo", true),
    );
    expect(await sistema.findByText(/Ligada: o Claude não edita/)).toBeInTheDocument();
    expect(sistema.getByRole("button", { name: "Desligar a proteção" })).toBeInTheDocument();
  });

  it("descadastra só depois da confirmação, e a falha mantém o cadastro", async () => {
    const api = apiConfiguracoesFalsa();
    api.descadastrar.mockRejectedValueOnce(new Error("settings.json inválido"));
    const { aoMudarSistemas } = montar(api);
    const sistema = await cartao();
    fireEvent.click(sistema.getByRole("button", { name: "Descadastrar o sistema" }));
    const confirmar = within(
      sistema.getByRole("region", { name: "Confirmar o descadastro de Faturamento Exemplo" }),
    );
    expect(confirmar.getByText(/Os documentos do sistema na base de conhecimento ficam/)).toBeInTheDocument();
    const botao = confirmar.getByRole("button", { name: "Descadastrar" });
    expect(botao).toBeDisabled();
    fireEvent.click(confirmar.getByRole("checkbox"));
    fireEvent.click(botao);
    expect(await sistema.findByText(/Não foi possível descadastrar/)).toBeInTheDocument();
    expect(aoMudarSistemas).not.toHaveBeenCalled();

    api.configuracoes.mockResolvedValue(configuracoesFicticias({ sistemas: [] }));
    fireEvent.click(botao);
    await waitFor(() => expect(api.descadastrar).toHaveBeenCalledTimes(2));
    expect(await screen.findByText("Nenhum sistema cadastrado.")).toBeInTheDocument();
    expect(aoMudarSistemas).toHaveBeenCalled();
  });

  it("abre os textos e as regras do sistema no editor", async () => {
    const { api } = montar();
    const sistema = await cartao();
    fireEvent.click(
      await sistema.findByRole("button", {
        name: "Abrir Regras do sistema Faturamento Exemplo no editor",
      }),
    );
    await waitFor(() =>
      expect(api.abrirTexto).toHaveBeenCalledWith("faturamento-exemplo/regras-do-sistema.md"),
    );
    fireEvent.click(screen.getByRole("button", { name: "Abrir Perguntar ao sistema no editor" }));
    await waitFor(() =>
      expect(api.abrirTexto).toHaveBeenCalledWith("_templates/roteiros/perguntar.md"),
    );
    expect(
      await screen.findByText(`Aberto: ${BASE}/_templates/roteiros/perguntar.md`, { exact: false }),
    ).toBeInTheDocument();
  });

  it("sem base, não lista textos e avisa a falha de leitura", async () => {
    const api = apiConfiguracoesFalsa(configuracoesFicticias({ base: null, sistemas: [] }));
    montar(api);
    expect(await screen.findByText("Ainda não escolhida.")).toBeInTheDocument();
    expect(screen.queryByRole("region", { name: "Textos editáveis" })).not.toBeInTheDocument();
    expect(api.textos).not.toHaveBeenCalled();

    const quebrada = apiConfiguracoesFalsa();
    quebrada.configuracoes.mockRejectedValue(new Error("configuração ilegível"));
    render(<TelaConfiguracoes api={quebrada} />);
    expect(await screen.findByText(/configuração ilegível/)).toBeInTheDocument();
  });
});
