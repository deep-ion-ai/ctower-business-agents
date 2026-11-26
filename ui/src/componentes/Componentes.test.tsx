import { fireEvent, render, screen, within } from "@testing-library/react";
import { Alerta, EstadoVazio } from "./Avisos";
import { Cartao, ItemLista, ListaItens, Secao, TituloSecao } from "./Secao";
import { Selo, tomDoSignificado } from "./Selo";

describe("Selo (#406)", () => {
  it("o tom vem do significado, nunca da tela", () => {
    expect(tomDoSignificado("Proposta do agente")).toBe("agente");
    expect(tomDoSignificado("aberta")).toBe("andamento");
    expect(tomDoSignificado(" aguardando revisão ")).toBe("atencao");
    expect(tomDoSignificado("confirmada")).toBe("concluido");
    expect(tomDoSignificado("não confere")).toBe("erro");
    expect(tomDoSignificado("14 em aberto")).toBe("neutro");
  });

  it("sem tom, usa o do texto; com tom, o dado", () => {
    render(
      <>
        <Selo>proposta do agente</Selo>
        <Selo tom="erro">falha</Selo>
      </>,
    );
    expect(screen.getByText("proposta do agente")).toHaveAttribute("data-tom", "agente");
    expect(screen.getByText("proposta do agente")).toHaveClass("selo");
    expect(screen.getByText("falha")).toHaveAttribute("data-tom", "erro");
  });
});

describe("Seção, cartão e item de lista (#406)", () => {
  it("a seção é uma região nomeada pelo título, com as ações ao lado", () => {
    render(
      <Secao titulo="Registros" acoes={<button type="button">Novo registro</button>}>
        <p>conteúdo</p>
      </Secao>,
    );
    const secao = screen.getByRole("region", { name: "Registros" });
    expect(within(secao).getByRole("heading", { name: "Registros", level: 2 })).toBeInTheDocument();
    expect(within(secao).getByRole("button", { name: "Novo registro" })).toBeInTheDocument();
  });

  it("TituloSecao aceita o nível do cabeçalho", () => {
    render(<TituloSecao nivel={3}>Pendências</TituloSecao>);
    expect(screen.getByRole("heading", { name: "Pendências", level: 3 })).toBeInTheDocument();
  });

  it("cartão nomeado é um article; item de lista tem metadado e ações", () => {
    render(
      <>
        <Cartao rotulo="Pergunta 1">texto</Cartao>
        <ListaItens rotulo="Linha do tempo">
          <ItemLista meta="hoje" acoes={<button type="button">Abrir</button>}>
            Ata aprovada
          </ItemLista>
        </ListaItens>
      </>,
    );
    expect(screen.getByRole("article", { name: "Pergunta 1" })).toHaveClass("bloco-cartao");
    const lista = screen.getByRole("list", { name: "Linha do tempo" });
    const item = within(lista).getByRole("listitem");
    expect(item).toHaveTextContent("Ata aprovada");
    expect(within(item).getByText("hoje")).toHaveClass("item-lista-meta");
  });
});

describe("EstadoVazio e Alerta (#406)", () => {
  it("estado vazio: uma frase e o próximo passo", () => {
    const criar = vi.fn();
    const { container } = render(
      <EstadoVazio acao={{ texto: "Criar o primeiro", aoEscolher: criar }}>Nenhum registro deste tipo.</EstadoVazio>,
    );
    expect(container.firstElementChild).toHaveClass("estado-vazio");
    expect(container).toHaveTextContent("Nenhum registro deste tipo. Criar o primeiro");
    fireEvent.click(screen.getByRole("button", { name: "Criar o primeiro" }));
    expect(criar).toHaveBeenCalled();
  });

  it("alerta de erro é anunciado (role alert); o de atenção é status; ícone fora da leitura", () => {
    render(
      <>
        <Alerta tom="erro" titulo="Não foi possível salvar.">
          Tente de novo.
        </Alerta>
        <Alerta tom="atencao" titulo="Código alterado." acoes={<button type="button">Revalidar</button>}>
          Revise a análise.
        </Alerta>
      </>,
    );
    const erro = screen.getByRole("alert");
    expect(erro).toHaveTextContent("Não foi possível salvar. Tente de novo.");
    expect(erro).toHaveAttribute("data-tom", "erro");
    expect(erro.querySelector("svg")).toHaveAttribute("aria-hidden", "true");
    const atencao = screen.getByRole("status");
    expect(within(atencao).getByText("Código alterado.").tagName).toBe("STRONG");
    expect(within(atencao).getByRole("button", { name: "Revalidar" })).toBeInTheDocument();
  });

  it("o papel do alerta pode ser trocado", () => {
    render(<Alerta tom="atencao" titulo="Só leitura." role="note" />);
    expect(screen.getByRole("note")).toHaveTextContent("Só leitura.");
  });
});
