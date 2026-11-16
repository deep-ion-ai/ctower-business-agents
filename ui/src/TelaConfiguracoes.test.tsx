import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { TelaConfiguracoes } from "./TelaConfiguracoes";
import { clientesProntos } from "./test/falsos";
import {
  BASE,
  TEXTOS,
  apiConfiguracoesFalsa,
  configuracoesFicticias,
  padraoFalso,
} from "./test/configuracoesFalso";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(), Channel: vi.fn() }));

// A janela do Tauri: o teste dispara o pedido de fechamento por `janela.aoFechar`.
const janela = vi.hoisted(() => ({
  aoFechar: null as null | ((e: { preventDefault: () => void }) => void),
  destroy: vi.fn(() => Promise.resolve()),
}));
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    onCloseRequested: (f: (e: { preventDefault: () => void }) => void) => {
      janela.aoFechar = f;
      return Promise.resolve(() => {
        janela.aoFechar = null;
      });
    },
    destroy: janela.destroy,
  }),
}));

const CHAVE_SECAO = "cba.configuracoes.secao";
const TITULO_MODELOS = "Modelos, roteiros e prompts";

beforeEach(() => {
  localStorage.clear();
});

/** Monta a tela já na seção `secao` (a que a tela lembra): `aparencia` é a inicial. */
function montar(api = apiConfiguracoesFalsa(), secao?: string) {
  if (secao) localStorage.setItem(CHAVE_SECAO, secao);
  const aoMudarProjetos = vi.fn();
  const vista = render(<TelaConfiguracoes api={api} aoMudarProjetos={aoMudarProjetos} />);
  return { api, aoMudarProjetos, ...vista };
}

const PROJETO_ABERTO = "projeto:faturamento-exemplo";

async function cartao() {
  return within(await screen.findByRole("article", { name: "Faturamento Exemplo" }));
}

const NOME_DO_MENU = "Seções das configurações";

/** O menu lateral; os `find*` esperam a tela terminar de carregar. */
function menu() {
  const dentro = () => within(screen.getByRole("navigation", { name: NOME_DO_MENU }));
  return {
    getByRole: (...a: Parameters<ReturnType<typeof dentro>["getByRole"]>) => dentro().getByRole(...a),
    queryByRole: (...a: Parameters<ReturnType<typeof dentro>["queryByRole"]>) =>
      dentro().queryByRole(...a),
    getByText: (...a: Parameters<ReturnType<typeof dentro>["getByText"]>) => dentro().getByText(...a),
    findByRole: async (...a: Parameters<ReturnType<typeof dentro>["getByRole"]>) =>
      waitFor(() => dentro().getByRole(...a)),
    findByText: async (...a: Parameters<ReturnType<typeof dentro>["getByText"]>) =>
      waitFor(() => dentro().getByText(...a)),
  };
}

async function trocarPasta(destino: string) {
  fireEvent.click(await screen.findByRole("button", { name: "Trocar a pasta…" }));
  const dialogo = within(await screen.findByRole("dialog"));
  fireEvent.change(dialogo.getByLabelText(/Pasta da base \(caminho completo\)/), {
    target: { value: destino },
  });
  fireEvent.click(dialogo.getByRole("button", { name: "Continuar" }));
  return dialogo;
}

describe("TelaConfiguracoes", () => {
  describe("menu lateral", () => {
    it("tem os grupos App, Base de conhecimento e Projetos, com o Acessos oculto", async () => {
      montar();
      await screen.findByRole("heading", { name: "Aparência" });
      const m = menu();
      expect(m.getByRole("group", { name: "App" })).toBeInTheDocument();
      expect(m.getByRole("group", { name: "Base de conhecimento" })).toBeInTheDocument();
      expect(m.getByRole("group", { name: "Projetos" })).toBeInTheDocument();
      for (const nome of ["Aparência", "Assistente de IA", "Pasta da base", "Modelos e prompts"]) {
        expect(m.getByRole("button", { name: nome })).toBeInTheDocument();
      }
      expect(m.getByRole("button", { name: "Faturamento Exemplo" })).toBeInTheDocument();
      expect(m.getByRole("button", { name: "+ Novo projeto" })).toBeEnabled();
      // “Acessos” espera a #193: o item existe no código, mas não aparece.
      expect(m.queryByRole("button", { name: "Acessos" })).not.toBeInTheDocument();
    });

    it("marca o item aberto com aria-current e mostra só a seção escolhida", async () => {
      montar();
      const m = menu();
      expect(await m.findByRole("button", { name: "Aparência" })).toHaveAttribute(
        "aria-current",
        "page",
      );
      expect(m.getByRole("button", { name: "Pasta da base" })).not.toHaveAttribute("aria-current");

      fireEvent.click(m.getByRole("button", { name: "Pasta da base" }));
      expect(m.getByRole("button", { name: "Pasta da base" })).toHaveAttribute(
        "aria-current",
        "page",
      );
      expect(m.getByRole("button", { name: "Aparência" })).not.toHaveAttribute("aria-current");
      expect(
        screen.getByRole("heading", { name: "Pasta da base de conhecimento" }),
      ).toBeInTheDocument();
      expect(screen.queryByRole("heading", { name: "Aparência", level: 2 })).not.toBeInTheDocument();
      expect(screen.queryByRole("heading", { name: TITULO_MODELOS })).not.toBeInTheDocument();

      fireEvent.click(m.getByRole("button", { name: "Faturamento Exemplo" }));
      expect(m.getByRole("button", { name: "Faturamento Exemplo" })).toHaveAttribute(
        "aria-current",
        "page",
      );
      expect(await screen.findByRole("article", { name: "Faturamento Exemplo" })).toBeInTheDocument();
    });

    it("lembra a seção ao sair e voltar para as configurações", async () => {
      const api = apiConfiguracoesFalsa();
      const primeira = montar(api);
      fireEvent.click(await menu().findByRole("button", { name: "Modelos e prompts" }));
      primeira.unmount();

      montar(api);
      expect(await menu().findByRole("button", { name: "Modelos e prompts" })).toHaveAttribute(
        "aria-current",
        "page",
      );
      expect(screen.getByRole("heading", { name: TITULO_MODELOS })).toBeInTheDocument();
      expect(localStorage.getItem(CHAVE_SECAO)).toBe("modelos");
    });

    it("lembra o projeto aberto e cai na Aparência se ele deixou de existir", async () => {
      const api = apiConfiguracoesFalsa();
      const primeira = montar(api);
      fireEvent.click(await menu().findByRole("button", { name: "Faturamento Exemplo" }));
      expect(localStorage.getItem(CHAVE_SECAO)).toBe(PROJETO_ABERTO);
      primeira.unmount();
      montar(api);
      expect(await screen.findByRole("article", { name: "Faturamento Exemplo" })).toBeInTheDocument();

      // Outra base, sem esse projeto: a tela não fica vazia.
      localStorage.setItem(CHAVE_SECAO, "projeto:que-sumiu");
      montar(apiConfiguracoesFalsa(configuracoesFicticias({ projetos: [] })));
      expect(await screen.findAllByRole("heading", { name: "Aparência", level: 2 })).not.toHaveLength(0);
    });

    it("ignora uma seção lembrada que não existe ou está oculta", async () => {
      localStorage.setItem(CHAVE_SECAO, "acessos");
      montar();
      expect(await menu().findByRole("button", { name: "Aparência" })).toHaveAttribute(
        "aria-current",
        "page",
      );
    });

    it("anda pelos itens com as setas e pelo teclado com Tab e Enter", async () => {
      montar();
      const m = menu();
      const aparencia = await m.findByRole("button", { name: "Aparência" });
      aparencia.focus();
      fireEvent.keyDown(aparencia, { key: "ArrowDown" });
      expect(m.getByRole("button", { name: "Assistente de IA" })).toHaveFocus();
      fireEvent.keyDown(m.getByRole("button", { name: "Assistente de IA" }), { key: "ArrowDown" });
      expect(m.getByRole("button", { name: "Pasta da base" })).toHaveFocus();
      fireEvent.keyDown(m.getByRole("button", { name: "Pasta da base" }), { key: "ArrowUp" });
      expect(m.getByRole("button", { name: "Assistente de IA" })).toHaveFocus();
      // Itens são botões de verdade: Tab e Enter/Espaço do navegador valem.
      expect(m.getByRole("button", { name: "Assistente de IA" }).tagName).toBe("BUTTON");
    });

    it("mostra as pendências de cada projeto no menu", async () => {
      const api = apiConfiguracoesFalsa();
      api.verificarAmbiente.mockResolvedValue({
        pronto: false,
        pendencias: ["servidor MCP ausente", "hooks ausentes"],
      });
      montar(api);
      const item = await menu().findByRole("button", { name: /Faturamento Exemplo/ });
      await waitFor(() => expect(item).toHaveTextContent("2"));
      expect(within(item).getByText("pendências")).toBeInTheDocument();
    });

    it("sem pendências, o projeto não mostra contagem", async () => {
      montar();
      const item = await menu().findByRole("button", { name: "Faturamento Exemplo" });
      await waitFor(() => expect(item.querySelector(".ponto-status")).toHaveAttribute("data-estado", "ok"));
      expect(item.querySelector(".menu-config-contagem")).toBeNull();
    });

    it("+ Novo projeto abre o assistente de cadastro", async () => {
      montar();
      fireEvent.click(await menu().findByRole("button", { name: "+ Novo projeto" }));
      expect(await screen.findByRole("dialog")).toBeInTheDocument();
    });
  });

  describe("Aparência", () => {
    it("aplica o tema na hora, com confirmação discreta", async () => {
      montar();
      await screen.findByRole("heading", { name: "Aparência", level: 2 });
      const temas = within(screen.getByRole("group", { name: "Tema" }));
      expect(temas.getAllByRole("radio")).toHaveLength(3);
      fireEvent.click(temas.getByRole("radio", { name: /Escuro/ }));
      expect(document.documentElement.dataset.theme).toBe("dark");
      expect(localStorage.getItem("cba.theme")).toBe("dark");
      expect(screen.getByText("Tema “Escuro” aplicado.")).toBeInTheDocument();
      fireEvent.click(temas.getByRole("radio", { name: /Claro/ }));
      expect(document.documentElement.dataset.theme).toBe("light");
      expect(screen.getByText("Tema “Claro” aplicado.")).toBeInTheDocument();
      expect(temas.getByRole("radio", { name: /Claro/ })).toBeChecked();
    });
  });

  describe("Assistente de IA", () => {
    it("mostra o estado do cliente escolhido: instalação, versão e conta", async () => {
      montar(undefined, "assistente");
      expect(await screen.findByRole("heading", { name: "Assistente de IA", level: 2 })).toBeInTheDocument();
      // O cliente gravado é o Claude Desktop.
      expect(screen.getByRole("radio", { name: /Claude Desktop/ })).toBeChecked();
      expect(await screen.findByText("✓ Instalado")).toBeInTheDocument();
      expect(screen.queryByText(/versão 2.1.0/)).not.toBeInTheDocument();
    });

    it("troca na própria tela, sem o passo do primeiro uso", async () => {
      const { api } = montar(undefined, "assistente");
      fireEvent.click(await screen.findByRole("radio", { name: /terminal do app/ }));
      await waitFor(() => expect(api.escolherCliente).toHaveBeenCalledWith("terminalEmbutido"));
      await waitFor(() => expect(api.configuracoes).toHaveBeenCalledTimes(2));
      expect(await screen.findByText(/Salvo\. A troca vale para as próximas conversas/)).toBeInTheDocument();
      expect(await screen.findByText("✓ Instalado · versão 2.1.0")).toBeInTheDocument();
      expect(screen.getByText("✓ Conta conectada")).toBeInTheDocument();
      expect(screen.queryByText("Como você prefere conversar com a IA?")).not.toBeInTheDocument();
    });

    it("explica por que o Claude Desktop está indisponível", async () => {
      const api = apiConfiguracoesFalsa();
      api.estadoClientes.mockResolvedValue(
        clientesProntos({
          claudeDesktop: { instalacao: { estado: "naoSuportado" }, orientacao: [] },
        }),
      );
      montar(api, "assistente");
      expect(
        await screen.findByText(/Indisponível neste computador: o Claude Desktop só existe/),
      ).toBeInTheDocument();
      expect(screen.getByRole("radio", { name: /Claude Desktop/ })).toBeDisabled();
    });

    it("mostra o que falta e o passo a passo quando o cliente não está pronto", async () => {
      const api = apiConfiguracoesFalsa(configuracoesFicticias({ cliente: "terminalEmbutido" }));
      api.estadoClientes.mockResolvedValue(
        clientesProntos({
          claudeCode: {
            instalacao: { estado: "naoEncontrado" },
            login: { estado: "naoVerificado" },
            orientacao: [{ texto: "Instale o Claude Code pelo site oficial." }],
          },
        }),
      );
      montar(api, "assistente");
      expect(await screen.findByText(/Instale o Claude Code seguindo o passo a passo/)).toBeInTheDocument();
      expect(screen.getByText("Instale o Claude Code pelo site oficial.")).toBeInTheDocument();
      expect(screen.getByText("Não instalado")).toBeInTheDocument();
    });

    it("Verificar de novo detecta os clientes outra vez", async () => {
      const { api } = montar(undefined, "assistente");
      await screen.findByText("✓ Instalado");
      expect(api.estadoClientes).toHaveBeenCalledTimes(1);
      fireEvent.click(screen.getByRole("button", { name: "Verificar de novo" }));
      await waitFor(() => expect(api.estadoClientes).toHaveBeenCalledTimes(2));
    });

    it("a falha ao gravar aparece e a escolha não muda", async () => {
      const api = apiConfiguracoesFalsa();
      api.escolherCliente.mockRejectedValueOnce(new Error("config.toml somente leitura"));
      montar(api, "assistente");
      fireEvent.click(await screen.findByRole("radio", { name: /terminal do app/ }));
      expect(await screen.findByText(/somente leitura/)).toBeInTheDocument();
      expect(screen.getByRole("radio", { name: /Claude Desktop/ })).toBeChecked();
    });
  });

  describe("Pasta da base", () => {
    it("mostra o caminho, os documentos, o git e o índice, e abre no computador", async () => {
      const { api } = montar(undefined, "pasta");
      expect(await screen.findByText(BASE)).toBeInTheDocument();
      expect(await screen.findByText("✓ 142 documentos · 3,3 MB")).toBeInTheDocument();
      expect(screen.getByText("versionada com git")).toBeInTheDocument();
      expect(screen.getByText(/índice atualizado/)).toBeInTheDocument();
      fireEvent.click(screen.getByRole("button", { name: "Abrir no computador" }));
      await waitFor(() => expect(api.abrirBase).toHaveBeenCalledTimes(1));
      expect(await screen.findByText("Pasta aberta no gerenciador de arquivos.")).toBeInTheDocument();
    });

    it("a falha ao abrir a pasta aparece", async () => {
      const api = apiConfiguracoesFalsa();
      api.abrirBase.mockRejectedValueOnce(new Error("não foi possível abrir a pasta"));
      montar(api, "pasta");
      fireEvent.click(await screen.findByRole("button", { name: "Abrir no computador" }));
      expect(await screen.findByText(/não foi possível abrir a pasta/)).toBeInTheDocument();
    });

    it("sem base, diz o que falta e oferece escolher a pasta", async () => {
      const api = apiConfiguracoesFalsa(configuracoesFicticias({ base: null, projetos: [] }));
      montar(api, "pasta");
      expect(await screen.findByText("Ainda não escolhida.")).toBeInTheDocument();
      expect(screen.queryByRole("button", { name: "Abrir no computador" })).not.toBeInTheDocument();
      expect(screen.getByRole("button", { name: "Escolher a pasta…" })).toBeInTheDocument();
      expect(api.infoBase).not.toHaveBeenCalled();
    });

    it("move a base só depois da confirmação, em dois passos", async () => {
      const { api, aoMudarProjetos } = montar(undefined, "pasta");
      fireEvent.click(await screen.findByRole("button", { name: "Trocar a pasta…" }));
      const dialogo = within(await screen.findByRole("dialog"));
      expect(dialogo.getByText("Passo 1 de 2")).toBeInTheDocument();
      fireEvent.change(dialogo.getByLabelText(/Pasta da base \(caminho completo\)/), {
        target: { value: "/Users/analista/OneDrive/base" },
      });
      fireEvent.click(dialogo.getByRole("button", { name: "Continuar" }));
      expect(await dialogo.findByText("Passo 2 de 2")).toBeInTheDocument();
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
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
      expect(aoMudarProjetos).toHaveBeenCalled();
    });

    it("só reaponta quando a pasta nova já tem arquivos, avisando das conversas abertas", async () => {
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
      montar(api, "pasta");
      await trocarPasta("/outra/base");
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

    it("Voltar leva ao passo 1 e Esc ou Cancelar fecham sem trocar nada", async () => {
      const api = apiConfiguracoesFalsa();
      montar(api, "pasta");
      const botao = await screen.findByRole("button", { name: "Trocar a pasta…" });
      botao.focus(); // o clique do navegador foca o botão; o jsdom não
      await trocarPasta("/outra/base");
      const confirmar = within(
        await screen.findByRole("region", { name: "Confirmar a troca da base" }),
      );
      fireEvent.click(confirmar.getByRole("button", { name: "← Voltar" }));
      expect(await screen.findByText("Passo 1 de 2")).toBeInTheDocument();

      fireEvent.keyDown(screen.getByRole("dialog"), { key: "Escape" });
      await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
      expect(api.trocarBase).not.toHaveBeenCalled();
      expect(botao).toHaveFocus();
    });

    it("a escolha da pasta mostra a recusa da base", async () => {
      const api = apiConfiguracoesFalsa();
      api.avaliarBase.mockResolvedValue({
        caminho: "/tmp/base",
        estado: "nova",
        recusa: "a base não pode ficar numa pasta temporária",
        git: false,
      });
      montar(api, "pasta");
      fireEvent.click(await screen.findByRole("button", { name: "Trocar a pasta…" }));
      fireEvent.change(screen.getByLabelText(/Pasta da base \(caminho completo\)/), {
        target: { value: "/tmp/base" },
      });
      expect(await screen.findByText(/pasta temporária/, {}, { timeout: 2000 })).toBeInTheDocument();
      expect(screen.getByRole("button", { name: "Continuar" })).toBeDisabled();
      expect(api.avaliarTroca).not.toHaveBeenCalled();
    });
  });

  describe("Modelos e prompts", () => {
    function grupo(nome: string) {
      return within(screen.getByRole("region", { name: nome }));
    }

    it("lista os grupos com descrição e estado de cada item", async () => {
      montar(undefined, "modelos");
      await screen.findByRole("heading", { name: TITULO_MODELOS });
      const modelos = grupo("Modelos dos documentos");
      expect(modelos.getByText("editado por você")).toBeInTheDocument();
      expect(modelos.getByText(/O documento com o que muda/)).toBeInTheDocument();
      const roteiros = grupo("Roteiros dos fluxos");
      expect(roteiros.getByText("padrão do app")).toBeInTheDocument();
      expect(roteiros.getByText(/Como o agente responde a uma pergunta/)).toBeInTheDocument();
      expect(roteiros.getByRole("button", { name: "Editar Perguntar ao sistema" })).toHaveTextContent(
        "Editar",
      );
      expect(grupo("Prompts prontos").getByText("cópia sem mudanças")).toBeInTheDocument();
      // As regras do projeto ficam na página do projeto, não aqui.
      expect(screen.queryByText(/Regras do projeto Faturamento Exemplo/)).not.toBeInTheDocument();
    });

    it("o filtro Só os editados esconde o que usa o padrão", async () => {
      montar(undefined, "modelos");
      await screen.findByRole("heading", { name: TITULO_MODELOS });
      const filtro = screen.getByRole("button", { name: "Só os editados" });
      expect(filtro).toHaveAttribute("aria-pressed", "false");
      fireEvent.click(filtro);
      expect(filtro).toHaveAttribute("aria-pressed", "true");
      expect(screen.getByText("Análise de impacto")).toBeInTheDocument();
      expect(screen.queryByText("Perguntar ao sistema")).not.toBeInTheDocument();
      expect(screen.queryByRole("region", { name: "Prompts prontos" })).not.toBeInTheDocument();
      fireEvent.click(filtro);
      expect(screen.getByText("Perguntar ao sistema")).toBeInTheDocument();
    });

    it("as ações secundárias ficam no menu ⋯: abrir no computador", async () => {
      const { api } = montar(undefined, "modelos");
      fireEvent.click(await screen.findByRole("button", { name: "Mais ações para Perguntar ao sistema" }));
      expect(screen.getByRole("menu", { name: "Mais ações para Perguntar ao sistema" })).toBeInTheDocument();
      // Sem cópia na base: o app cria a cópia e abre.
      fireEvent.click(screen.getByRole("menuitem", { name: "Criar a cópia e abrir no computador" }));
      await waitFor(() =>
        expect(api.abrirTexto).toHaveBeenCalledWith("_templates/roteiros/perguntar.md"),
      );
      expect(
        await screen.findByText(`Aberto: ${BASE}/_templates/roteiros/perguntar.md`, { exact: false }),
      ).toBeInTheDocument();
      expect(screen.queryByRole("menu")).not.toBeInTheDocument();
    });

    it("o menu ⋯ fecha com Esc devolvendo o foco, e anda com as setas", async () => {
      montar(undefined, "modelos");
      const botao = await screen.findByRole("button", { name: "Mais ações para Análise de impacto" });
      fireEvent.click(botao);
      const itens = screen.getAllByRole("menuitem");
      expect(itens[0]).toHaveFocus();
      expect(botao).toHaveAttribute("aria-expanded", "true");
      fireEvent.keyDown(itens[0], { key: "ArrowDown" });
      expect(itens[1]).toHaveFocus();
      fireEvent.keyDown(itens[1], { key: "Escape" });
      expect(screen.queryByRole("menu")).not.toBeInTheDocument();
      expect(botao).toHaveFocus();
      expect(botao).toHaveAttribute("aria-expanded", "false");
    });

    it("volta ao padrão do app só depois de confirmar", async () => {
      const { api } = montar(undefined, "modelos");
      fireEvent.click(await screen.findByRole("button", { name: "Mais ações para Análise de impacto" }));
      fireEvent.click(screen.getByRole("menuitem", { name: "Voltar ao padrão do app" }));
      const dialogo = within(await screen.findByRole("dialog"));
      expect(api.restaurarTexto).not.toHaveBeenCalled();
      fireEvent.click(dialogo.getByRole("button", { name: "Manter a minha versão" }));
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
      expect(api.restaurarTexto).not.toHaveBeenCalled();

      fireEvent.click(screen.getByRole("button", { name: "Mais ações para Análise de impacto" }));
      fireEvent.click(screen.getByRole("menuitem", { name: "Voltar ao padrão do app" }));
      fireEvent.click(
        within(await screen.findByRole("dialog")).getByRole("button", { name: "Voltar ao padrão" }),
      );
      await waitFor(() =>
        expect(api.restaurarTexto).toHaveBeenCalledWith("_templates/analise-impacto.md"),
      );
      await waitFor(() =>
        expect(
          within(screen.getByText("Análise de impacto").closest("li")!).getByText("padrão do app"),
        ).toBeInTheDocument(),
      );
    });

    it("um texto que já é o padrão não oferece voltar ao padrão", async () => {
      montar(undefined, "modelos");
      fireEvent.click(await screen.findByRole("button", { name: "Mais ações para Perguntar ao sistema" }));
      expect(screen.getByRole("menuitem", { name: "Voltar ao padrão do app" })).toBeDisabled();
    });

    it("sem base, diz por que está vazia e leva à pasta da base", async () => {
      const api = apiConfiguracoesFalsa(configuracoesFicticias({ base: null, projetos: [] }));
      montar(api, "modelos");
      expect(await screen.findByText(/Sem a pasta da base, não há onde guardar/)).toBeInTheDocument();
      expect(api.textos).not.toHaveBeenCalled();
      expect(screen.queryByRole("button", { name: "Só os editados" })).not.toBeInTheDocument();
      fireEvent.click(screen.getByRole("button", { name: "Ir para a pasta da base" }));
      expect(
        await screen.findByRole("heading", { name: "Pasta da base de conhecimento" }),
      ).toBeInTheDocument();
      expect(menu().getByRole("button", { name: "Pasta da base" })).toHaveAttribute("aria-current", "page");
    });
  });

  describe("Projetos", () => {
    it("sem base, + Novo projeto fica desligado e diz por quê", async () => {
      const api = apiConfiguracoesFalsa(configuracoesFicticias({ base: null, projetos: [] }));
      montar(api);
      expect(await menu().findByRole("button", { name: "+ Novo projeto" })).toBeDisabled();
      expect(menu().getByText("Nenhum projeto cadastrado.")).toBeInTheDocument();
      expect(menu().getByText(/Escolha a pasta da base para cadastrar projetos/)).toBeInTheDocument();
    });

    it("mostra a pasta, o ambiente e a proteção do projeto escolhido", async () => {
      montar(undefined, PROJETO_ABERTO);
      const projeto = await cartao();
      expect(projeto.getByText("/codigo/faturamento-exemplo")).toBeInTheDocument();
      expect(await projeto.findByText("Servidor MCP “cba”: registrado")).toBeInTheDocument();
      expect(projeto.getByText("Hooks: 2 de 2 registrados")).toBeInTheDocument();
      expect(await projeto.findByText(/Desligada \(padrão\)/)).toBeInTheDocument();
      expect(screen.getByText(/O ambiente do Claude fica em/)).toBeInTheDocument();
    });

    it("ajusta as exclusões com a prévia do inventário", async () => {
      const { api, aoMudarProjetos } = montar(undefined, PROJETO_ABERTO);
      const projeto = await cartao();
      fireEvent.click(projeto.getByText("Exclusões: o que fica fora da análise"));
      const campo = projeto.getByLabelText(/Excluir também/);
      expect(campo).toHaveValue("docs/antigos/");
      fireEvent.change(campo, { target: { value: "docs/antigos/\ncobol/" } });
      fireEvent.click(projeto.getByRole("button", { name: "Ver o que será analisado" }));
      await waitFor(() =>
        expect(api.previaExclusoes).toHaveBeenCalledWith({
          projeto: "faturamento-exemplo",
          exclusoes: ["docs/antigos/", "cobol/"],
          inclusoes: [],
        }),
      );
      // Reincluir um item ignorado pela prévia.
      fireEvent.click(await projeto.findByRole("button", { name: "Incluir .env na análise" }));
      await waitFor(() =>
        expect(api.previaExclusoes).toHaveBeenLastCalledWith(
          expect.objectContaining({ inclusoes: ["/.env"] }),
        ),
      );
      fireEvent.click(projeto.getByRole("button", { name: "Salvar as exclusões" }));
      await waitFor(() =>
        expect(api.salvarExclusoes).toHaveBeenCalledWith({
          projeto: "faturamento-exemplo",
          exclusoes: ["docs/antigos/", "cobol/"],
          inclusoes: ["/.env"],
        }),
      );
      expect(await projeto.findByText(/Exclusões salvas/)).toBeInTheDocument();
      expect(aoMudarProjetos).toHaveBeenCalled();
    });

    it("desfaz o ambiente com confirmação e verifica de novo", async () => {
      const { api } = montar(undefined, PROJETO_ABERTO);
      const projeto = await cartao();
      // O menu, o detalhe e o estado do ambiente verificam uma vez cada.
      await waitFor(() => expect(api.verificarAmbiente).toHaveBeenCalledTimes(3));
      fireEvent.click(projeto.getByRole("button", { name: "Desfazer o ambiente" }));
      const confirmar = within(
        projeto.getByRole("region", { name: "Confirmar desfazer o ambiente" }),
      );
      fireEvent.click(confirmar.getByRole("button", { name: "Desfazer" }));
      await waitFor(() => expect(api.desfazerAmbiente).toHaveBeenCalledWith("faturamento-exemplo"));
      expect(await projeto.findByText(/Ambiente desfeito/)).toBeInTheDocument();
      await waitFor(() => expect(api.verificarAmbiente).toHaveBeenCalledTimes(5));
      expect(api.situacaoProtecao).toHaveBeenCalledTimes(2);
    });

    it("o menu confere as pendências de novo quando o ambiente muda", async () => {
      const api = apiConfiguracoesFalsa();
      montar(api, PROJETO_ABERTO);
      const projeto = await cartao();
      await waitFor(() => expect(api.verificarAmbiente).toHaveBeenCalledTimes(3));
      const item = menu().getByRole("button", { name: /Faturamento Exemplo/ });
      expect(item.querySelector(".menu-config-contagem")).toBeNull();

      api.verificarAmbiente.mockResolvedValue({ pronto: false, pendencias: ["hooks ausentes"] });
      fireEvent.click(projeto.getByRole("button", { name: "Desfazer o ambiente" }));
      fireEvent.click(
        within(projeto.getByRole("region", { name: "Confirmar desfazer o ambiente" })).getByRole(
          "button",
          { name: "Desfazer" },
        ),
      );
      await waitFor(() => expect(item.querySelector(".menu-config-contagem")).not.toBeNull());
      expect(item).toHaveTextContent("1");
    });

    it("liga a proteção mostrando as regras antes", async () => {
      const { api } = montar(undefined, PROJETO_ABERTO);
      const projeto = await cartao();
      fireEvent.click(await projeto.findByRole("button", { name: "Ligar a proteção" }));
      expect(projeto.getByText("Edit(//codigo/faturamento-exemplo/**)")).toBeInTheDocument();
      expect(api.mudarProtecao).not.toHaveBeenCalled();
      fireEvent.click(projeto.getByRole("button", { name: "Concordo, ligar" }));
      await waitFor(() =>
        expect(api.mudarProtecao).toHaveBeenCalledWith("faturamento-exemplo", true),
      );
      expect(await projeto.findByText(/Ligada: o Claude não edita/)).toBeInTheDocument();
      expect(projeto.getByRole("button", { name: "Desligar a proteção" })).toBeInTheDocument();
    });

    it("descadastra só depois da confirmação, e a falha mantém o cadastro", async () => {
      const api = apiConfiguracoesFalsa();
      api.descadastrar.mockRejectedValueOnce(new Error("settings.json inválido"));
      const { aoMudarProjetos } = montar(api, PROJETO_ABERTO);
      const projeto = await cartao();
      fireEvent.click(projeto.getByRole("button", { name: "Descadastrar o projeto" }));
      const confirmar = within(
        projeto.getByRole("region", { name: "Confirmar o descadastro de Faturamento Exemplo" }),
      );
      expect(
        confirmar.getByText(/Os documentos do projeto na base de conhecimento ficam/),
      ).toBeInTheDocument();
      const botao = confirmar.getByRole("button", { name: "Descadastrar" });
      expect(botao).toBeDisabled();
      fireEvent.click(confirmar.getByRole("checkbox"));
      fireEvent.click(botao);
      expect(await projeto.findByText(/Não foi possível descadastrar/)).toBeInTheDocument();
      expect(aoMudarProjetos).not.toHaveBeenCalled();

      api.configuracoes.mockResolvedValue(configuracoesFicticias({ projetos: [] }));
      fireEvent.click(botao);
      await waitFor(() => expect(api.descadastrar).toHaveBeenCalledTimes(2));
      // Sem projeto, a tela volta para a Aparência e o menu avisa.
      expect(await menu().findByText("Nenhum projeto cadastrado.")).toBeInTheDocument();
      expect(menu().getByRole("button", { name: "Aparência" })).toHaveAttribute("aria-current", "page");
      expect(aoMudarProjetos).toHaveBeenCalled();
    });

    it("abre as regras do projeto no editor do computador", async () => {
      const { api } = montar(undefined, PROJETO_ABERTO);
      const projeto = await cartao();
      fireEvent.click(
        await projeto.findByRole("button", {
          name: "Abrir Regras do projeto Faturamento Exemplo no editor do computador",
        }),
      );
      await waitFor(() =>
        expect(api.abrirTexto).toHaveBeenCalledWith("faturamento-exemplo/regras-do-projeto.md"),
      );
    });
  });

  it("avisa a falha de leitura das configurações", async () => {
    const quebrada = apiConfiguracoesFalsa();
    quebrada.configuracoes.mockRejectedValue(new Error("configuração ilegível"));
    render(<TelaConfiguracoes api={quebrada} />);
    expect(await screen.findByText(/configuração ilegível/)).toBeInTheDocument();
  });

  describe("editor de textos", () => {
    const ROTEIRO = TEXTOS[1]; // sem cópia na base
    const TEMPLATE = TEXTOS[0]; // editado

    async function editar(api = apiConfiguracoesFalsa(), rotulo = ROTEIRO.rotulo) {
      montar(api, "modelos");
      fireEvent.click(await screen.findByRole("button", { name: `Editar ${rotulo}` }));
      const campo = await screen.findByRole("textbox", { name: "Texto" });
      return { api, campo: campo as HTMLTextAreaElement };
    }

    function linhaDaTabela(rotulo: string) {
      return within(screen.getByText(rotulo).closest("li")!);
    }

    it("abre o editor com rótulo, caminho, texto e prévia com nota e chips", async () => {
      const { campo } = await editar();
      expect(campo.value).toBe(padraoFalso(ROTEIRO));
      const editor = within(screen.getByRole("region", { name: `Editor de ${ROTEIRO.rotulo}` }));
      expect(editor.getByRole("heading", { name: ROTEIRO.rotulo })).toBeInTheDocument();
      expect(editor.getByText(ROTEIRO.arquivo)).toBeInTheDocument();
      // As configurações ficam escondidas, não desmontadas.
      expect(screen.queryByRole("heading", { name: TITULO_MODELOS })).not.toBeInTheDocument();

      const previa = within(screen.getByRole("region", { name: "Prévia" }));
      // A prévia usa useDeferredValue: pode chegar um render depois.
      expect(await previa.findByRole("heading", { level: 1 })).toHaveTextContent("{{id}} · Perguntar");
      expect(previa.getAllByText("{{corpo}}")[0]).toHaveClass("chip-variavel");
      const nota = within(screen.getByLabelText("Instrução para quem edita"));
      expect(nota.getByText(/Instrução: use/)).toBeInTheDocument();
      expect(previa.queryByText(/<!--/)).not.toBeInTheDocument();
      // Sem alterações: nada a salvar.
      expect(screen.getByRole("button", { name: "Salvar" })).toBeDisabled();
    });

    it("edita, salva com Ctrl+S e a tabela reflete o estado ao voltar", async () => {
      const { api, campo } = await editar();
      fireEvent.change(campo, { target: { value: `${campo.value}\nMais uma linha.\n` } });
      expect(screen.getByText("Alterações não salvas")).toBeInTheDocument();
      // A prévia acompanha a digitação.
      expect(await screen.findByText("Mais uma linha.")).toBeInTheDocument();

      fireEvent.keyDown(window, { key: "s", ctrlKey: true });
      await waitFor(() =>
        expect(api.salvarTexto).toHaveBeenCalledWith(
          ROTEIRO.arquivo,
          `${padraoFalso(ROTEIRO)}\nMais uma linha.\n`,
          "ausente",
        ),
      );
      expect(await screen.findByText("Salvo.")).toBeInTheDocument();
      expect(screen.queryByText("Alterações não salvas")).not.toBeInTheDocument();
      expect(screen.getByRole("button", { name: "Salvar" })).toBeDisabled();

      // Sem pendência, voltar não pergunta.
      fireEvent.click(screen.getByRole("button", { name: "Voltar" }));
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
      await screen.findByRole("heading", { name: TITULO_MODELOS });
      await waitFor(() =>
        expect(
          linhaDaTabela(ROTEIRO.rotulo).getByText("editado por você"),
        ).toBeInTheDocument(),
      );
    });

    it("sair com alteração pendente pede confirmação", async () => {
      const { api, campo } = await editar();
      fireEvent.change(campo, { target: { value: "outro texto" } });

      fireEvent.click(screen.getByRole("button", { name: "Voltar" }));
      let dialogo = within(await screen.findByRole("dialog", { name: "Alterações não salvas" }));
      fireEvent.click(dialogo.getByRole("button", { name: "Continuar editando" }));
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
      expect(campo).toHaveValue("outro texto");

      fireEvent.click(screen.getByRole("button", { name: "Voltar" }));
      dialogo = within(await screen.findByRole("dialog", { name: "Alterações não salvas" }));
      fireEvent.click(dialogo.getByRole("button", { name: "Descartar e voltar" }));
      await screen.findByRole("heading", { name: TITULO_MODELOS });
      expect(api.salvarTexto).not.toHaveBeenCalled();
      expect(linhaDaTabela(ROTEIRO.rotulo).getByText("padrão do app")).toBeInTheDocument();
    });

    it("salvar e voltar grava e sai", async () => {
      const { api, campo } = await editar();
      fireEvent.change(campo, { target: { value: "outro texto" } });
      fireEvent.click(screen.getByRole("button", { name: "Voltar" }));
      fireEvent.click(await screen.findByRole("button", { name: "Salvar e voltar" }));
      await waitFor(() => expect(api.salvarTexto).toHaveBeenCalledTimes(1));
      await screen.findByRole("heading", { name: TITULO_MODELOS });
      await waitFor(() =>
        expect(
          linhaDaTabela(ROTEIRO.rotulo).getByText("editado por você"),
        ).toBeInTheDocument(),
      );
    });

    it("descartar volta ao texto salvo, depois de confirmar", async () => {
      const { campo } = await editar();
      const original = campo.value;
      fireEvent.change(campo, { target: { value: "rascunho" } });
      fireEvent.click(screen.getByRole("button", { name: "Descartar alterações" }));
      fireEvent.click(
        within(await screen.findByRole("dialog")).getByRole("button", { name: "Descartar" }),
      );
      expect(campo).toHaveValue(original);
      expect(screen.queryByText("Alterações não salvas")).not.toBeInTheDocument();
    });

    it("conflito: oferece sobrescrever, que salva de novo com a versão atual", async () => {
      const api = apiConfiguracoesFalsa();
      const { campo } = await editar(api, TEMPLATE.rotulo);
      fireEvent.change(campo, { target: { value: "meu texto" } });
      api.editarFora(TEMPLATE.arquivo, "texto de outro programa");

      fireEvent.click(screen.getByRole("button", { name: "Salvar" }));
      const dialogo = within(
        await screen.findByRole("dialog", { name: "O arquivo mudou fora do app" }),
      );
      expect(api.salvarTexto).toHaveBeenCalledTimes(1);
      fireEvent.click(dialogo.getByRole("button", { name: "Sobrescrever com o meu texto" }));
      await waitFor(() => expect(api.salvarTexto).toHaveBeenCalledTimes(2));
      expect(api.salvarTexto.mock.calls[1][1]).toBe("meu texto");
      expect(api.salvarTexto.mock.calls[1][2]).not.toBe(api.salvarTexto.mock.calls[0][2]);
      expect(await screen.findByText("Salvo.")).toBeInTheDocument();
      expect(api.textoNaBase(TEMPLATE.arquivo)).toBe("meu texto");
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    });

    it("conflito: recarregar traz o texto do arquivo e descarta o do editor", async () => {
      const api = apiConfiguracoesFalsa();
      const { campo } = await editar(api, TEMPLATE.rotulo);
      fireEvent.change(campo, { target: { value: "meu texto" } });
      api.editarFora(TEMPLATE.arquivo, "texto de outro programa");

      fireEvent.click(screen.getByRole("button", { name: "Salvar" }));
      fireEvent.click(await screen.findByRole("button", { name: "Recarregar o arquivo" }));
      await waitFor(() => expect(campo).toHaveValue("texto de outro programa"));
      expect(screen.queryByText("Alterações não salvas")).not.toBeInTheDocument();
      expect(api.textoNaBase(TEMPLATE.arquivo)).toBe("texto de outro programa");
    });

    it("restaura o padrão só depois de confirmar", async () => {
      const api = apiConfiguracoesFalsa();
      const { campo } = await editar(api, TEMPLATE.rotulo);
      fireEvent.click(screen.getByRole("button", { name: "Restaurar o padrão" }));
      const dialogo = within(await screen.findByRole("dialog", { name: "Restaurar o padrão" }));
      fireEvent.click(dialogo.getByRole("button", { name: "Cancelar" }));
      expect(api.restaurarTexto).not.toHaveBeenCalled();

      fireEvent.click(screen.getByRole("button", { name: "Restaurar o padrão" }));
      fireEvent.click(
        within(await screen.findByRole("dialog")).getByRole("button", {
          name: "Restaurar o padrão",
        }),
      );
      await waitFor(() => expect(api.restaurarTexto).toHaveBeenCalledWith(TEMPLATE.arquivo));
      await waitFor(() => expect(campo).toHaveValue(padraoFalso(TEMPLATE)));
      fireEvent.click(screen.getByRole("button", { name: "Voltar" }));
      await screen.findByRole("heading", { name: TITULO_MODELOS });
      await waitFor(() =>
        expect(linhaDaTabela(TEMPLATE.rotulo).getByText("padrão do app")).toBeInTheDocument(),
      );
    });

    it("abrir no editor do computador funciona no editor", async () => {
      const { api } = await editar();
      fireEvent.click(screen.getByRole("button", { name: "Abrir no editor do computador" }));
      await waitFor(() => expect(api.abrirTexto).toHaveBeenCalledWith(ROTEIRO.arquivo));
      expect(
        await screen.findByText(`Aberto no editor do computador: ${BASE}/${ROTEIRO.arquivo}.`),
      ).toBeInTheDocument();
      // A cópia foi criada: o editor releu o arquivo para a versão acompanhar.
      await waitFor(() => expect(api.lerTexto).toHaveBeenCalledTimes(2));
    });

    it("lista as variáveis, insere no cursor e mostra os avisos com link para a linha", async () => {
      const { api, campo } = await editar();
      const lateral = within(screen.getByRole("complementary", { name: "Variáveis e avisos" }));
      expect(lateral.getByText("Nenhum aviso.")).toBeInTheDocument();

      campo.focus();
      campo.setSelectionRange(0, 0);
      fireEvent.click(lateral.getByRole("button", { name: "{{titulo}}" }));
      expect(campo.value.startsWith("{{titulo}}<!--")).toBe(true);
      expect(screen.getByText("Alterações não salvas")).toBeInTheDocument();

      fireEvent.change(campo, { target: { value: "# Título\n\n{{inventado}}\n" } });
      // Com debounce: a validação roda depois da pausa na digitação.
      expect(
        await lateral.findByText(/marcador desconhecido/, undefined, { timeout: 3000 }),
      ).toBeInTheDocument();
      expect(api.validarTexto).toHaveBeenLastCalledWith(
        ROTEIRO.arquivo,
        "# Título\n\n{{inventado}}\n",
      );
      // O chip que o texto não aceita é destacado na prévia.
      const previa = within(screen.getByRole("region", { name: "Prévia" }));
      await waitFor(() =>
        expect(previa.getByText("{{inventado}}")).toHaveAttribute("data-desconhecido", "true"),
      );

      fireEvent.click(lateral.getByRole("button", { name: "Linha 3" }));
      expect(campo.selectionStart).toBe("# Título\n\n".length);
      expect(campo.selectionEnd).toBe("# Título\n\n{{inventado}}".length);

      fireEvent.click(screen.getByRole("button", { name: "Ocultar painel" }));
      expect(
        screen.queryByRole("complementary", { name: "Variáveis e avisos" }),
      ).not.toBeInTheDocument();
    });

    it("alterna entre Editar e Prévia pelas abas", async () => {
      await editar();
      const abas = within(screen.getByRole("tablist", { name: "Painéis do editor" }));
      expect(abas.getByRole("tab", { name: "Editar" })).toHaveAttribute("aria-selected", "true");
      fireEvent.click(abas.getByRole("tab", { name: "Prévia" }));
      expect(abas.getByRole("tab", { name: "Prévia" })).toHaveAttribute("aria-selected", "true");
      expect(screen.getByRole("region", { name: "Prévia" }).closest("[data-aba]")).toHaveAttribute(
        "data-aba",
        "previa",
      );
    });

    it("avisa ao fechar a janela com alterações pendentes", async () => {
      janela.destroy.mockClear();
      const { campo } = await editar();
      expect(janela.aoFechar).toBeNull();
      fireEvent.change(campo, { target: { value: "rascunho" } });
      await waitFor(() => expect(janela.aoFechar).not.toBeNull());

      const preventDefault = vi.fn();
      janela.aoFechar!({ preventDefault });
      expect(preventDefault).toHaveBeenCalled();
      const dialogo = within(
        await screen.findByRole("dialog", { name: "Fechar com alterações não salvas" }),
      );
      fireEvent.click(dialogo.getByRole("button", { name: "Continuar editando" }));
      expect(janela.destroy).not.toHaveBeenCalled();

      janela.aoFechar!({ preventDefault });
      fireEvent.click(
        within(await screen.findByRole("dialog")).getByRole("button", {
          name: "Descartar e fechar",
        }),
      );
      expect(janela.destroy).toHaveBeenCalledTimes(1);
    });

    it("regras do projeto: editor sem variáveis, aberto pelo cartão do projeto", async () => {
      const api = apiConfiguracoesFalsa();
      montar(api, "projeto:faturamento-exemplo");
      const projeto = await cartao();
      fireEvent.click(
        await projeto.findByRole("button", {
          name: "Editar Regras do projeto Faturamento Exemplo",
        }),
      );
      await screen.findByRole("textbox", { name: "Texto" });
      expect(api.lerTexto).toHaveBeenCalledWith("faturamento-exemplo/regras-do-projeto.md");
      expect(screen.getByText(/Este texto é livre/)).toBeInTheDocument();
    });

    it("renderiza a prévia de um roteiro de 500 linhas", async () => {
      const { campo } = await editar();
      const longo = Array.from(
        { length: 500 },
        (_, i) => `Linha ${i + 1} com {{id}} e **destaque**`,
      ).join("\n\n");
      fireEvent.change(campo, { target: { value: longo } });
      const previa = within(screen.getByRole("region", { name: "Prévia" }));
      await waitFor(() => expect(previa.getAllByText("{{id}}")).toHaveLength(500));
    });
  });
});
