import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { TelaConfiguracoes } from "./TelaConfiguracoes";
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

function montar(api = apiConfiguracoesFalsa()) {
  const aoMudarProjetos = vi.fn();
  render(<TelaConfiguracoes api={api} aoMudarProjetos={aoMudarProjetos} />);
  return { api, aoMudarProjetos };
}

async function cartao() {
  return within(await screen.findByRole("article", { name: "Faturamento Exemplo" }));
}

describe("TelaConfiguracoes", () => {
  it("mostra o cliente, a base, os projetos e os textos editáveis", async () => {
    montar();
    const cliente = within(await screen.findByRole("region", { name: "Cliente de IA" }));
    expect(cliente.getByText("Claude Desktop")).toBeInTheDocument();
    expect(
      within(screen.getByRole("region", { name: "Pasta da base" })).getByText(BASE),
    ).toBeInTheDocument();
    const projeto = await cartao();
    expect(projeto.getByText("/codigo/faturamento-exemplo")).toBeInTheDocument();
    expect(await projeto.findByText("Servidor MCP “cba”: registrado")).toBeInTheDocument();
    expect(projeto.getByText("Hooks: 2 de 2 registrados")).toBeInTheDocument();
    expect(await projeto.findByText(/Desligada \(padrão\)/)).toBeInTheDocument();

    const textos = within(screen.getByRole("region", { name: "Textos editáveis" }));
    const templates = within(textos.getByRole("group", { name: "Templates dos documentos" }));
    expect(templates.getByText("editado na base (sobrepõe o padrão)")).toBeInTheDocument();
    const roteiros = within(textos.getByRole("group", { name: "Roteiros dos fluxos" }));
    expect(roteiros.getByText("padrão do app")).toBeInTheDocument();
    expect(roteiros.getByRole("button", { name: "Editar Perguntar ao sistema" })).toHaveTextContent(
      "Editar",
    );
    expect(
      roteiros.getByRole("button", { name: "Abrir Perguntar ao sistema no editor do computador" }),
    ).toHaveTextContent("Criar a cópia e abrir no computador");
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
    const { api, aoMudarProjetos } = montar();
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
    expect(aoMudarProjetos).toHaveBeenCalled();
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
    const { api, aoMudarProjetos } = montar();
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
    const { api } = montar();
    const projeto = await cartao();
    await waitFor(() => expect(api.verificarAmbiente).toHaveBeenCalledTimes(2));
    fireEvent.click(projeto.getByRole("button", { name: "Desfazer o ambiente" }));
    const confirmar = within(
      projeto.getByRole("region", { name: "Confirmar desfazer o ambiente" }),
    );
    fireEvent.click(confirmar.getByRole("button", { name: "Desfazer" }));
    await waitFor(() => expect(api.desfazerAmbiente).toHaveBeenCalledWith("faturamento-exemplo"));
    expect(await projeto.findByText(/Ambiente desfeito/)).toBeInTheDocument();
    await waitFor(() => expect(api.verificarAmbiente).toHaveBeenCalledTimes(4));
    expect(api.situacaoProtecao).toHaveBeenCalledTimes(2);
  });

  it("liga a proteção mostrando as regras antes", async () => {
    const { api } = montar();
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
    const { aoMudarProjetos } = montar(api);
    const projeto = await cartao();
    fireEvent.click(projeto.getByRole("button", { name: "Descadastrar o projeto" }));
    const confirmar = within(
      projeto.getByRole("region", { name: "Confirmar o descadastro de Faturamento Exemplo" }),
    );
    expect(confirmar.getByText(/Os documentos do projeto na base de conhecimento ficam/)).toBeInTheDocument();
    const botao = confirmar.getByRole("button", { name: "Descadastrar" });
    expect(botao).toBeDisabled();
    fireEvent.click(confirmar.getByRole("checkbox"));
    fireEvent.click(botao);
    expect(await projeto.findByText(/Não foi possível descadastrar/)).toBeInTheDocument();
    expect(aoMudarProjetos).not.toHaveBeenCalled();

    api.configuracoes.mockResolvedValue(configuracoesFicticias({ projetos: [] }));
    fireEvent.click(botao);
    await waitFor(() => expect(api.descadastrar).toHaveBeenCalledTimes(2));
    expect(await screen.findByText("Nenhum projeto cadastrado.")).toBeInTheDocument();
    expect(aoMudarProjetos).toHaveBeenCalled();
  });

  it("abre os textos e as regras do projeto no editor do computador", async () => {
    const { api } = montar();
    const projeto = await cartao();
    fireEvent.click(
      await projeto.findByRole("button", {
        name: "Abrir Regras do projeto Faturamento Exemplo no editor do computador",
      }),
    );
    await waitFor(() =>
      expect(api.abrirTexto).toHaveBeenCalledWith("faturamento-exemplo/regras-do-projeto.md"),
    );
    fireEvent.click(screen.getByRole("button", {
        name: "Abrir Perguntar ao sistema no editor do computador",
      }));
    await waitFor(() =>
      expect(api.abrirTexto).toHaveBeenCalledWith("_templates/roteiros/perguntar.md"),
    );
    expect(
      await screen.findByText(`Aberto: ${BASE}/_templates/roteiros/perguntar.md`, { exact: false }),
    ).toBeInTheDocument();
  });

  it("sem base, não lista textos e avisa a falha de leitura", async () => {
    const api = apiConfiguracoesFalsa(configuracoesFicticias({ base: null, projetos: [] }));
    montar(api);
    expect(await screen.findByText("Ainda não escolhida.")).toBeInTheDocument();
    expect(screen.queryByRole("region", { name: "Textos editáveis" })).not.toBeInTheDocument();
    expect(api.textos).not.toHaveBeenCalled();

    const quebrada = apiConfiguracoesFalsa();
    quebrada.configuracoes.mockRejectedValue(new Error("configuração ilegível"));
    render(<TelaConfiguracoes api={quebrada} />);
    expect(await screen.findByText(/configuração ilegível/)).toBeInTheDocument();
  });
  describe("editor de textos", () => {
    const ROTEIRO = TEXTOS[1]; // sem cópia na base
    const TEMPLATE = TEXTOS[0]; // editado

    async function editar(api = apiConfiguracoesFalsa(), rotulo = ROTEIRO.rotulo) {
      montar(api);
      fireEvent.click(await screen.findByRole("button", { name: `Editar ${rotulo}` }));
      const campo = await screen.findByRole("textbox", { name: "Texto" });
      return { api, campo: campo as HTMLTextAreaElement };
    }

    function linhaDaTabela(rotulo: string) {
      const textos = within(screen.getByRole("region", { name: "Textos editáveis" }));
      return within(textos.getByText(rotulo).closest("tr")!);
    }

    it("abre o editor com rótulo, caminho, texto e prévia com nota e chips", async () => {
      const { campo } = await editar();
      expect(campo.value).toBe(padraoFalso(ROTEIRO));
      const editor = within(screen.getByRole("region", { name: `Editor de ${ROTEIRO.rotulo}` }));
      expect(editor.getByRole("heading", { name: ROTEIRO.rotulo })).toBeInTheDocument();
      expect(editor.getByText(ROTEIRO.arquivo)).toBeInTheDocument();
      // As configurações ficam escondidas, não desmontadas.
      expect(screen.queryByRole("region", { name: "Textos editáveis" })).not.toBeInTheDocument();

      const previa = within(screen.getByRole("region", { name: "Prévia" }));
      expect(previa.getByRole("heading", { level: 1 })).toHaveTextContent("{{id}} · Perguntar");
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
      await screen.findByRole("region", { name: "Textos editáveis" });
      await waitFor(() =>
        expect(
          linhaDaTabela(ROTEIRO.rotulo).getByText("editado na base (sobrepõe o padrão)"),
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
      await screen.findByRole("region", { name: "Textos editáveis" });
      expect(api.salvarTexto).not.toHaveBeenCalled();
      expect(linhaDaTabela(ROTEIRO.rotulo).getByText("padrão do app")).toBeInTheDocument();
    });

    it("salvar e voltar grava e sai", async () => {
      const { api, campo } = await editar();
      fireEvent.change(campo, { target: { value: "outro texto" } });
      fireEvent.click(screen.getByRole("button", { name: "Voltar" }));
      fireEvent.click(await screen.findByRole("button", { name: "Salvar e voltar" }));
      await waitFor(() => expect(api.salvarTexto).toHaveBeenCalledTimes(1));
      await screen.findByRole("region", { name: "Textos editáveis" });
      await waitFor(() =>
        expect(
          linhaDaTabela(ROTEIRO.rotulo).getByText("editado na base (sobrepõe o padrão)"),
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
      await screen.findByRole("region", { name: "Textos editáveis" });
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
      montar(api);
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
