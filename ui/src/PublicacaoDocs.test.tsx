import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { PublicacaoDocs } from "./PublicacaoDocs";
import type { ApiPublicacao, PreviaPublicacao, Publicacao } from "./publicacao";

const PROJETO = "faturamento-exemplo";

function previa(extra: Partial<PreviaPublicacao> = {}): PreviaPublicacao {
  return {
    destino: "docs/negocio em GitHub exemplo/faturamento",
    repositorio: "GitHub exemplo/faturamento",
    independente: false,
    base: "main",
    branch: "docs/cba-faturamento-exemplo",
    aprovados: 3,
    mudancas: [
      { caminho: "docs/negocio/analises/A-0001.md", tipo: "novo" },
      { caminho: "docs/negocio/glossario/carencia.md", tipo: "novo" },
      { caminho: "docs/negocio/regras/RN-COB-001.md", tipo: "alterado" },
      { caminho: "docs/negocio/regras/RN-COB-002.md", tipo: "removido" },
    ],
    pendente_na_branch: false,
    nada_a_publicar: false,
    ...extra,
  };
}

function api(p: PreviaPublicacao, publicado?: Partial<Publicacao["publicado"]>): ApiPublicacao {
  return {
    previa: vi.fn().mockResolvedValue(p),
    publicar: vi.fn().mockResolvedValue({
      previa: p,
      publicado: {
        situacao: "aberto",
        branch: p.branch,
        base: p.base,
        commit: "abc123",
        link: "https://github.com/exemplo/faturamento/pull/1",
        ...publicado,
      },
    }),
  };
}

describe("Publicar a documentação aprovada (Faturamento Exemplo)", () => {
  it("mostra a prévia e só publica depois da conferência de segredos e dados pessoais", async () => {
    const a = api(previa());
    const abrirUrl = vi.fn();
    render(<PublicacaoDocs api={a} projeto={PROJETO} abrirUrl={abrirUrl} />);
    expect(screen.getByRole("status")).toHaveTextContent(/Conferindo/);
    expect(await screen.findByText("docs/negocio em GitHub exemplo/faturamento")).toBeInTheDocument();
    expect(a.previa).toHaveBeenCalledWith(PROJETO);
    const novos = screen.getByRole("list", { name: "Novos" });
    expect(within(novos).getAllByRole("listitem")).toHaveLength(2);
    expect(within(screen.getByRole("list", { name: "Alterados" })).getByText(/RN-COB-001/)).toBeInTheDocument();
    expect(within(screen.getByRole("list", { name: "Removidos" })).getByText(/RN-COB-002/)).toBeInTheDocument();
    expect(screen.getByRole("note")).toHaveTextContent(/segredos.*dados pessoais/s);

    const publicar = screen.getByRole("button", { name: "Publicar" });
    expect(publicar).toBeDisabled();
    fireEvent.click(screen.getByRole("checkbox", { name: /Conferi/ }));
    expect(publicar).toBeEnabled();
    fireEvent.click(publicar);
    await waitFor(() => expect(a.publicar).toHaveBeenCalledWith(PROJETO));
    expect(await screen.findByText(/o PR\/MR foi aberto/)).toBeInTheDocument();
    fireEvent.click(screen.getByRole("link", { name: "Abrir o PR/MR" }));
    expect(abrirUrl).toHaveBeenCalledWith("https://github.com/exemplo/faturamento/pull/1");
    expect(screen.queryByRole("button", { name: "Publicar" })).not.toBeInTheDocument();
  });

  it("sem mudanças, diz que não há nada a publicar e não oferece o botão", async () => {
    const a = api(previa({ mudancas: [], nada_a_publicar: true }));
    render(<PublicacaoDocs api={a} projeto={PROJETO} abrirUrl={vi.fn()} />);
    expect(await screen.findByText(/Nada a publicar/)).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Publicar" })).not.toBeInTheDocument();
  });

  it("republicar sem mudança efetiva mostra o resultado sem link novo", async () => {
    const a = api(previa({ mudancas: [], pendente_na_branch: true }), {
      situacao: "sem_mudancas",
      commit: null,
      link: null,
    });
    render(<PublicacaoDocs api={a} projeto={PROJETO} abrirUrl={vi.fn()} />);
    expect(await screen.findByText(/ainda não mesclada/)).toBeInTheDocument();
    fireEvent.click(screen.getByRole("checkbox", { name: /Conferi/ }));
    fireEvent.click(screen.getByRole("button", { name: "Publicar" }));
    expect(await screen.findByText(/Nada mudou/)).toBeInTheDocument();
    expect(screen.queryByRole("link")).not.toBeInTheDocument();
  });

  it("mostra o erro (sem destino) e tenta de novo", async () => {
    const a: ApiPublicacao = {
      previa: vi
        .fn()
        .mockRejectedValueOnce("o projeto não tem destino da documentação: cadastre-o na tela de repositórios")
        .mockResolvedValueOnce(previa()),
      publicar: vi.fn(),
    };
    render(<PublicacaoDocs api={a} projeto={PROJETO} abrirUrl={vi.fn()} />);
    expect(await screen.findByRole("alert")).toHaveTextContent(/destino da documentação/);
    fireEvent.click(screen.getByRole("button", { name: "Tentar de novo" }));
    expect(await screen.findByRole("list", { name: "Novos" })).toBeInTheDocument();
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });
});
