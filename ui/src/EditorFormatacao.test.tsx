import { fireEvent, render, screen, within } from "@testing-library/react";
import { EditorTexto } from "./EditorTexto";
import { TEXTOS, apiConfiguracoesFalsa } from "./test/configuracoesFalso";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(), Channel: vi.fn() }));
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    onCloseRequested: () => Promise.resolve(() => undefined),
    destroy: () => Promise.resolve(),
  }),
}));

const ROTEIRO = TEXTOS[1];

/** jsdom não implementa `execCommand`: um falso que aplica a inserção como o navegador. */
function falsoInsertText() {
  const exec = vi.fn((comando: string, _ui?: boolean, valor?: string) => {
    const campo = document.activeElement as HTMLTextAreaElement;
    if (comando !== "insertText" || !(campo instanceof HTMLTextAreaElement)) return false;
    campo.setRangeText(valor ?? "", campo.selectionStart, campo.selectionEnd, "end");
    campo.dispatchEvent(new Event("input", { bubbles: true }));
    return true;
  });
  Object.defineProperty(document, "execCommand", { value: exec, configurable: true, writable: true });
  return exec;
}

async function abrir() {
  const api = apiConfiguracoesFalsa();
  render(<EditorTexto api={api} arquivo={ROTEIRO.arquivo} aoSair={vi.fn()} aoAlterado={vi.fn()} />);
  const campo = (await screen.findByRole("textbox", { name: "Texto" })) as HTMLTextAreaElement;
  fireEvent.change(campo, { target: { value: "uma palavra aqui" } });
  return { campo, barra: within(screen.getByRole("toolbar", { name: "Formatação" })) };
}

function selecionar(campo: HTMLTextAreaElement, ini: number, fim = ini) {
  campo.focus();
  campo.setSelectionRange(ini, fim);
}

afterEach(() => {
  Reflect.deleteProperty(document, "execCommand");
});

describe("barra de formatação", () => {
  it("é uma toolbar com botões nomeados, com dica e atalho", async () => {
    const { barra } = await abrir();
    for (const nome of [
      "Título",
      "Negrito",
      "Itálico",
      "Lista",
      "Lista numerada",
      "Link",
      "Código",
      "Bloco de código",
      "Citação",
      "Tabela",
      "Ajuda",
    ]) {
      expect(barra.getByRole("button", { name: nome })).toBeInTheDocument();
    }
    expect(barra.getByRole("button", { name: "Negrito" })).toHaveAttribute("title", "Negrito (Ctrl+B)");
    expect(barra.getByRole("button", { name: "Tabela" })).toHaveAttribute("title", "Tabela");
  });

  const CASOS: [string, string, string][] = [
    // [botão, esperado com "palavra" selecionada, esperado sem seleção (cursor após "uma ")]
    ["Título", "## uma palavra aqui", "## uma palavra aqui"],
    ["Negrito", "uma **palavra** aqui", "uma **texto em negrito**palavra aqui"],
    ["Itálico", "uma *palavra* aqui", "uma *texto em itálico*palavra aqui"],
    ["Lista", "- uma palavra aqui", "- uma palavra aqui"],
    ["Lista numerada", "1. uma palavra aqui", "1. uma palavra aqui"],
    ["Link", "uma [palavra](https://) aqui", "uma [texto do link](https://)palavra aqui"],
    ["Código", "uma `palavra` aqui", "uma `código`palavra aqui"],
    ["Bloco de código", "uma \n```\npalavra\n```\n aqui", "uma \n```\ncódigo\n```\npalavra aqui"],
    ["Citação", "> uma palavra aqui", "> uma palavra aqui"],
    [
      "Tabela",
      "uma \n| palavra | Coluna 2 |\n| --- | --- |\n| valor | valor |\n aqui".replace(/\n aqui$/, " aqui"),
      "uma \n| Coluna 1 | Coluna 2 |\n| --- | --- |\n| valor | valor |palavra aqui",
    ],
  ];

  it.each(CASOS)("%s: com seleção", async (nome, esperado) => {
    const exec = falsoInsertText();
    const { campo, barra } = await abrir();
    selecionar(campo, 4, 11);
    fireEvent.click(barra.getByRole("button", { name: nome }));
    expect(exec).toHaveBeenCalledWith("insertText", false, expect.any(String));
    if (nome === "Tabela") {
      expect(campo.value).toContain("| palavra | Coluna 2 |");
    } else {
      expect(campo.value).toBe(esperado);
    }
  });

  it.each(CASOS)("%s: sem seleção", async (nome, _c, esperado) => {
    falsoInsertText();
    const { campo, barra } = await abrir();
    selecionar(campo, 4);
    fireEvent.click(barra.getByRole("button", { name: nome }));
    expect(campo.value).toBe(esperado);
  });

  it("seleciona o modelo inserido, para já digitar por cima", async () => {
    falsoInsertText();
    const { campo, barra } = await abrir();
    selecionar(campo, 0);
    fireEvent.click(barra.getByRole("button", { name: "Negrito" }));
    expect(campo.value.slice(campo.selectionStart, campo.selectionEnd)).toBe("texto em negrito");
    fireEvent.click(barra.getByRole("button", { name: "Link" }));
    expect(campo.selectionEnd).toBeGreaterThan(campo.selectionStart);
  });

  it("a inserção passa por execCommand('insertText'), que entra no desfazer", async () => {
    const exec = falsoInsertText();
    const { campo, barra } = await abrir();
    selecionar(campo, 4, 11);
    fireEvent.click(barra.getByRole("button", { name: "Negrito" }));
    expect(exec).toHaveBeenCalledTimes(1);
    expect(exec).toHaveBeenCalledWith("insertText", false, "**palavra**");
    // O trecho trocado era a seleção, não o texto todo: o desfazer restaura só ele.
    expect(campo.value).toBe("uma **palavra** aqui");
    expect(screen.getByText("Alterações não salvas")).toBeInTheDocument();
  });

  it("sem execCommand, cai em setRangeText e o texto muda do mesmo jeito", async () => {
    const { campo, barra } = await abrir();
    selecionar(campo, 4, 11);
    fireEvent.click(barra.getByRole("button", { name: "Itálico" }));
    expect(campo.value).toBe("uma *palavra* aqui");
  });

  it.each([
    ["b", "uma **palavra** aqui"],
    ["i", "uma *palavra* aqui"],
    ["k", "uma [palavra](https://) aqui"],
  ])("atalho Ctrl+%s no texto", async (tecla, esperado) => {
    const exec = falsoInsertText();
    const { campo } = await abrir();
    selecionar(campo, 4, 11);
    const livre = fireEvent.keyDown(campo, { key: tecla, ctrlKey: true });
    expect(livre).toBe(false); // preventDefault: o navegador não age
    expect(exec).toHaveBeenCalled();
    expect(campo.value).toBe(esperado);
  });

  it("atalho com Cmd (macOS) e ignorado fora do texto", async () => {
    falsoInsertText();
    const { campo } = await abrir();
    selecionar(campo, 4, 11);
    fireEvent.keyDown(campo, { key: "b", metaKey: true });
    expect(campo.value).toBe("uma **palavra** aqui");
    fireEvent.keyDown(screen.getByRole("button", { name: "Salvar" }), { key: "i", ctrlKey: true });
    expect(campo.value).toBe("uma **palavra** aqui");
  });
});

describe("ajuda de Markdown", () => {
  it("abre pelo botão Ajuda com os exemplos renderizados e o específico do app", async () => {
    const { barra } = await abrir();
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    fireEvent.click(barra.getByRole("button", { name: "Ajuda" }));
    const dialogo = screen.getByRole("dialog", { name: "Ajuda de formatação" });
    expect(dialogo).toHaveAttribute("aria-modal", "true");
    expect(dialogo).toHaveFocus();
    const ajuda = within(dialogo);
    expect(ajuda.getByRole("columnheader", { name: "Você escreve" })).toBeInTheDocument();
    expect(ajuda.getByRole("columnheader", { name: "Aparece assim" })).toBeInTheDocument();

    const como = (nome: string) => within(ajuda.getByRole("group", { name: `${nome}: como aparece` }));
    expect(como("Títulos").getByRole("heading", { level: 1, name: "Título" })).toBeInTheDocument();
    expect(como("Ênfase").getByText("negrito").tagName).toBe("STRONG");
    expect(como("Ênfase").getByText("itálico").tagName).toBe("EM");
    expect(como("Lista").getAllByRole("listitem").length).toBeGreaterThanOrEqual(2);
    expect(como("Lista numerada").getByRole("list").tagName).toBe("OL");
    expect(como("Link").getByText("texto do link")).toBeInTheDocument();
    expect(como("Código").getByText("código").tagName).toBe("CODE");
    expect(como("Bloco de código").getByText(/linha 1/).closest("pre")).not.toBeNull();
    expect(como("Citação").getByText("Texto citado").closest("blockquote")).not.toBeNull();
    expect(como("Tabela").getByRole("table")).toBeInTheDocument();
    expect(ajuda.getByRole("group", { name: "Linha horizontal: como aparece" }).querySelector("hr")).not.toBeNull();

    expect(ajuda.getByRole("heading", { name: "Específico deste app" })).toBeInTheDocument();
    expect(ajuda.getByText(/Comentário de instrução/)).toBeInTheDocument();
    expect(ajuda.getByText(/Desconhecida/)).toBeInTheDocument();
    expect(ajuda.getByText(/Essencial ausente/)).toBeInTheDocument();
  });

  it("Esc fecha e devolve o foco ao botão Ajuda", async () => {
    const { barra } = await abrir();
    const botao = barra.getByRole("button", { name: "Ajuda" });
    fireEvent.click(botao);
    const dialogo = screen.getByRole("dialog");
    fireEvent.keyDown(within(dialogo).getByRole("button", { name: "Fechar" }), { key: "Escape" });
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(botao).toHaveFocus();
  });

  it("fecha pelos botões Fechar e × e devolve o foco", async () => {
    const { barra } = await abrir();
    const botao = barra.getByRole("button", { name: "Ajuda" });
    for (const nome of ["Fechar", "Fechar a ajuda"]) {
      fireEvent.click(botao);
      fireEvent.click(within(screen.getByRole("dialog")).getByRole("button", { name: nome }));
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
      expect(botao).toHaveFocus();
    }
  });

  it("prende o foco: Tab no último volta ao primeiro e Shift+Tab no primeiro vai ao último", async () => {
    const { barra } = await abrir();
    fireEvent.click(barra.getByRole("button", { name: "Ajuda" }));
    const dialogo = screen.getByRole("dialog");
    const primeiro = within(dialogo).getByRole("button", { name: "Fechar a ajuda" });
    const ultimo = within(dialogo).getByRole("button", { name: "Fechar" });

    ultimo.focus();
    expect(fireEvent.keyDown(ultimo, { key: "Tab" })).toBe(false);
    expect(primeiro).toHaveFocus();
    expect(fireEvent.keyDown(primeiro, { key: "Tab", shiftKey: true })).toBe(false);
    expect(ultimo).toHaveFocus();
    // Do próprio contêiner (foco inicial), Shift+Tab também vai ao último.
    dialogo.focus();
    fireEvent.keyDown(dialogo, { key: "Tab", shiftKey: true });
    expect(ultimo).toHaveFocus();
    // No meio do caminho o navegador cuida do Tab: não é interceptado.
    primeiro.focus();
    expect(fireEvent.keyDown(primeiro, { key: "Tab" })).toBe(true);
  });

  it("Ctrl+S não salva por trás do modal", async () => {
    const api = apiConfiguracoesFalsa();
    render(<EditorTexto api={api} arquivo={ROTEIRO.arquivo} aoSair={vi.fn()} aoAlterado={vi.fn()} />);
    const campo = await screen.findByRole("textbox", { name: "Texto" });
    fireEvent.change(campo, { target: { value: "novo" } });
    fireEvent.click(screen.getByRole("button", { name: "Ajuda" }));
    fireEvent.keyDown(window, { key: "s", ctrlKey: true });
    expect(api.salvarTexto).not.toHaveBeenCalled();
  });
});
