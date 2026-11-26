import { fireEvent, render, screen, within } from "@testing-library/react";
import { IconeConfirmar } from "../icones";
import { BarraAcoes, MenuAcoes, type Acao } from "./Acoes";
import { BotaoIcone } from "./BotaoIcone";

function acoes(...textos: string[]): Acao[] {
  return textos.map((texto) => ({ texto, aoEscolher: vi.fn() }));
}

describe("MenuAcoes (#406)", () => {
  it("botão com ▾ e menu acessível: aria-haspopup, aria-expanded, menu e itens", () => {
    render(<MenuAcoes rotulo="Exportar" itens={acoes("PDF", "Word")} />);
    const botao = screen.getByRole("button", { name: "Exportar" });
    expect(botao).toHaveAttribute("aria-haspopup", "menu");
    expect(botao).toHaveAttribute("aria-expanded", "false");
    expect(botao).toHaveTextContent("▾");
    expect(screen.queryByRole("menu")).not.toBeInTheDocument();

    fireEvent.click(botao);
    expect(botao).toHaveAttribute("aria-expanded", "true");
    const menu = screen.getByRole("menu", { name: "Exportar" });
    expect(botao).toHaveAttribute("aria-controls", menu.id);
    expect(within(menu).getAllByRole("menuitem").map((i) => i.textContent)).toEqual(["PDF", "Word"]);
    expect(screen.getByRole("menuitem", { name: "PDF" })).toHaveFocus();
  });

  it("setas, Home e End andam pelos itens; Esc fecha e devolve o foco ao botão", () => {
    render(<MenuAcoes rotulo="Nova análise" itens={acoes("Impacto", "Histórias", "Protótipo")} />);
    const botao = screen.getByRole("button", { name: "Nova análise" });
    fireEvent.click(botao);
    const menu = screen.getByRole("menu");
    fireEvent.keyDown(menu, { key: "ArrowDown" });
    expect(screen.getByRole("menuitem", { name: "Histórias" })).toHaveFocus();
    fireEvent.keyDown(menu, { key: "End" });
    expect(screen.getByRole("menuitem", { name: "Protótipo" })).toHaveFocus();
    fireEvent.keyDown(menu, { key: "ArrowDown" });
    expect(screen.getByRole("menuitem", { name: "Impacto" })).toHaveFocus();
    fireEvent.keyDown(menu, { key: "ArrowUp" });
    expect(screen.getByRole("menuitem", { name: "Protótipo" })).toHaveFocus();
    fireEvent.keyDown(menu, { key: "Home" });
    expect(screen.getByRole("menuitem", { name: "Impacto" })).toHaveFocus();

    fireEvent.keyDown(menu, { key: "Escape" });
    expect(screen.queryByRole("menu")).not.toBeInTheDocument();
    expect(botao).toHaveAttribute("aria-expanded", "false");
    expect(botao).toHaveFocus();
  });

  it("seta para cima no botão abre no último item; o item escolhido fecha o menu e age", () => {
    const itens = acoes("A", "B");
    render(<MenuAcoes rotulo="Mais" itens={itens} />);
    fireEvent.keyDown(screen.getByRole("button", { name: "Mais" }), { key: "ArrowUp" });
    expect(screen.getByRole("menuitem", { name: "B" })).toHaveFocus();
    fireEvent.click(screen.getByRole("menuitem", { name: "B" }));
    expect(itens[1].aoEscolher).toHaveBeenCalled();
    expect(screen.queryByRole("menu")).not.toBeInTheDocument();
  });

  it("pula item desabilitado e fecha com clique fora", () => {
    render(
      <MenuAcoes
        rotulo="Mais"
        itens={[{ texto: "A", aoEscolher: vi.fn(), desabilitado: true }, ...acoes("B")]}
      />,
    );
    fireEvent.click(screen.getByRole("button", { name: "Mais" }));
    expect(screen.getByRole("menuitem", { name: "B" })).toHaveFocus();
    fireEvent.mouseDown(document.body);
    expect(screen.queryByRole("menu")).not.toBeInTheDocument();
  });

  it("só de ícone (⋯): o rótulo vira o nome acessível e a dica, sem a seta (#407)", () => {
    render(<MenuAcoes rotulo="Mais ações" icone={<IconeConfirmar />} itens={acoes("Abrir")} />);
    const botao = screen.getByRole("button", { name: "Mais ações" });
    expect(botao).toHaveAttribute("title", "Mais ações");
    expect(botao).toHaveClass("menu-acoes-icone");
    expect(botao).not.toHaveTextContent("▾");
    fireEvent.click(botao);
    expect(screen.getByRole("menu", { name: "Mais ações" })).toBeInTheDocument();
  });
});

describe("BarraAcoes (#406)", () => {
  it("uma principal e as secundárias num grupo nomeado", () => {
    const principal = { texto: "Analisar impacto", aoEscolher: vi.fn() };
    render(<BarraAcoes rotulo="Ações da demanda" principal={principal} secundarias={acoes("Editar", "Arquivar")} />);
    const grupo = screen.getByRole("group", { name: "Ações da demanda" });
    const botoes = within(grupo).getAllByRole("button");
    expect(botoes.map((b) => b.textContent)).toEqual(["Analisar impacto", "Editar", "Arquivar"]);
    expect(botoes[0]).toHaveClass("btn-primary");
    expect(botoes[1]).not.toHaveClass("btn-primary");
    fireEvent.click(botoes[0]);
    expect(principal.aoEscolher).toHaveBeenCalled();
  });

  it("mais de 3 secundárias viram um menu", () => {
    render(<BarraAcoes secundarias={acoes("A", "B", "C", "D")} rotuloMenu="Outras ações" />);
    expect(screen.queryByRole("button", { name: "A" })).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Outras ações" }));
    expect(screen.getAllByRole("menuitem")).toHaveLength(4);
  });

  it("ação destrutiva leva o tom de perigo", () => {
    render(<BarraAcoes secundarias={[{ texto: "Excluir", aoEscolher: vi.fn(), perigo: true }]} />);
    expect(screen.getByRole("button", { name: "Excluir" })).toHaveClass("btn-danger");
  });
});

describe("BotaoIcone (#406)", () => {
  it("o rótulo é o nome acessível e a dica; o ícone fica fora da leitura", () => {
    const aoClicar = vi.fn();
    const { container } = render(
      <BotaoIcone rotulo="Confirmar a pergunta" onClick={aoClicar} aria-expanded={false}>
        <IconeConfirmar />
      </BotaoIcone>,
    );
    const botao = screen.getByRole("button", { name: "Confirmar a pergunta" });
    expect(botao).toHaveAttribute("title", "Confirmar a pergunta");
    expect(botao).toHaveAttribute("aria-expanded", "false");
    expect(container.querySelector("svg")).toHaveAttribute("aria-hidden", "true");
    fireEvent.click(botao);
    expect(aoClicar).toHaveBeenCalled();
  });
});
