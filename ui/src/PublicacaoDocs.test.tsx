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
    paginas: 4,
    documentos: {
      documentos: [
        {
          tipo: "regra",
          id: "RN-COB-003",
          titulo: "Multa sobre a parcela",
          mudanca: "novo",
          pagina: "docs/negocio/regras/cobranca.md",
        },
        { tipo: "termo", id: "carencia", titulo: "Carência", mudanca: "novo", pagina: "docs/negocio/glossario.md" },
        {
          tipo: "regra",
          id: "RN-COB-001",
          titulo: "Multa de 2%",
          mudanca: "alterado",
          pagina: "docs/negocio/regras/cobranca.md",
        },
        {
          tipo: "regra",
          id: "RN-COB-002",
          titulo: "Sem multa até o vencimento",
          mudanca: "removido",
          pagina: "docs/negocio/regras/cobranca.md",
        },
      ],
      reformatados: 0,
      texto: "1 regra nova, 1 alterada, 1 removida; 1 termo novo",
    },
    mudancas: [
      { caminho: "docs/negocio/glossario.md", tipo: "novo" },
      { caminho: "docs/negocio/README.md", tipo: "alterado" },
      { caminho: "docs/negocio/regras/cobranca.md", tipo: "alterado" },
    ],
    formato_antigo: false,
    pendente_na_branch: false,
    branch_atualizada: false,
    nada_a_publicar: false,
    avisos: [],
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
    // Por documento, na língua do analista, e as páginas afetadas.
    expect(screen.getByText(/1 regra nova, 1 alterada, 1 removida; 1 termo novo/)).toBeInTheDocument();
    const novos = screen.getByRole("list", { name: "Documentos novos" });
    expect(within(novos).getAllByRole("listitem")).toHaveLength(2);
    expect(within(novos).getByText("RN-COB-003")).toBeInTheDocument();
    expect(
      within(screen.getByRole("list", { name: "Documentos alterados" })).getByText(/RN-COB-001/),
    ).toBeInTheDocument();
    expect(
      within(screen.getByRole("list", { name: "Documentos removidos" })).getByText(/RN-COB-002/),
    ).toBeInTheDocument();
    const paginas = screen.getByRole("list", { name: "Páginas afetadas" });
    expect(within(paginas).getAllByRole("listitem")).toHaveLength(3);
    expect(within(paginas).getByText("docs/negocio/README.md")).toBeInTheDocument();
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

  it("sem documento aprovado, diz que ainda não há aprovados", async () => {
    const a = api(
      previa({
        aprovados: 0,
        paginas: 0,
        mudancas: [],
        documentos: { documentos: [], reformatados: 0, texto: "" },
        nada_a_publicar: true,
      }),
    );
    render(<PublicacaoDocs api={a} projeto={PROJETO} abrirUrl={vi.fn()} />);
    expect(await screen.findByText("Ainda não há documentos aprovados neste projeto.")).toBeInTheDocument();
    expect(screen.queryByText(/repositório já tem/)).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Publicar" })).not.toBeInTheDocument();
  });

  it("com o PR/MR aberto já atualizado, avisa antes de publicar que não há o que enviar", async () => {
    const a = api(previa({ pendente_na_branch: true, branch_atualizada: true, nada_a_publicar: true }));
    render(<PublicacaoDocs api={a} projeto={PROJETO} abrirUrl={vi.fn()} />);
    expect(await screen.findByText(/já tem esta versão: não há nada a enviar/)).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Publicar" })).not.toBeInTheDocument();
    expect(a.publicar).not.toHaveBeenCalled();
  });

  it("na migração do formato antigo, explica que os arquivos antigos saem", async () => {
    const a = api(
      previa({
        formato_antigo: true,
        documentos: { documentos: [], reformatados: 3, texto: "3 documentos só mudam de formato" },
      }),
    );
    render(<PublicacaoDocs api={a} projeto={PROJETO} abrirUrl={vi.fn()} />);
    expect(await screen.findByText(/3 documentos só mudam de formato/)).toBeInTheDocument();
    expect(screen.getByText(/formato antigo/)).toHaveTextContent(/escritos à mão na pasta ficam/);
    expect(screen.getByRole("button", { name: "Publicar" })).toBeInTheDocument();
  });

  it("em janela baixa, o corpo rola e o aviso e o botão ficam no rodapé visível", async () => {
    // Janela de 600 px com o painel começando em 380 px: sobra pouco espaço.
    const alturaOriginal = window.innerHeight;
    Object.defineProperty(window, "innerHeight", { configurable: true, value: 600 });
    const retangulo = vi
      .spyOn(HTMLElement.prototype, "getBoundingClientRect")
      .mockReturnValue({ top: 380, bottom: 0, left: 0, right: 0, width: 0, height: 0, x: 0, y: 380 } as DOMRect);
    try {
      const muitos = Array.from({ length: 40 }, (_, i) => ({
        caminho: `docs/negocio/funcionalidades/f-${i}.md`,
        tipo: "novo" as const,
      }));
      render(<PublicacaoDocs api={api(previa({ mudancas: muitos }))} projeto={PROJETO} abrirUrl={vi.fn()} />);
      await screen.findByRole("list", { name: "Documentos novos" });
      const painel = screen.getByRole("region", { name: "Publicação da documentação" });
      // Do topo do painel até a borda de baixo, com a folga do padding.
      expect(painel.style.maxHeight).toBe("192px");
      const corpo = screen.getByTestId("publicacao-corpo");
      const acoes = screen.getByTestId("publicacao-acoes");
      // A lista longa fica no corpo (que rola); o aviso e o botão, fora dele, no rodapé.
      expect(corpo).toContainElement(screen.getByRole("list", { name: "Páginas afetadas" }));
      expect(corpo).not.toContainElement(screen.getByRole("button", { name: "Publicar" }));
      expect(acoes).toContainElement(screen.getByRole("button", { name: "Publicar" }));
      expect(acoes).toContainElement(screen.getByRole("note"));
      expect(painel.lastElementChild).toBe(acoes);
      // Ao redimensionar a janela, a altura acompanha.
      Object.defineProperty(window, "innerHeight", { configurable: true, value: 1080 });
      fireEvent(window, new Event("resize"));
      await waitFor(() => expect(painel.style.maxHeight).toBe("672px"));
    } finally {
      retangulo.mockRestore();
      Object.defineProperty(window, "innerHeight", { configurable: true, value: alturaOriginal });
    }
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
    expect(await screen.findByRole("list", { name: "Documentos novos" })).toBeInTheDocument();
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });
});
