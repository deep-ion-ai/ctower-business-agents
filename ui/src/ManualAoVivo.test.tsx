import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import type { Revisao } from "./AcoesRevisao";
import type { PassoManual } from "./analise";
import { ManualAoVivo } from "./ManualAoVivo";
import { comoPng, resolverImagem, type ApiManual } from "./manual";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(), Channel: vi.fn() }));

/** PNG 1×1. */
const PNG = new Uint8Array([
  0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
  0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f, 0x15, 0xc4,
  0x89, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0x00, 0x01, 0x00, 0x00,
  0x05, 0x00, 0x01, 0x0d, 0x0a, 0x2d, 0xb4, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae,
  0x42, 0x60, 0x82,
]);

/** Passos do manual de "Cancelar fatura" do Faturamento Exemplo. */
function passos(): PassoManual[] {
  return [
    {
      numero: 1,
      titulo: "Abra a tela de cancelamento",
      instrucao: "No menu **Faturas**, escolha *Cancelar*.",
      tela: "Cancelamento de fatura",
      resultado: "A tela mostra as faturas abertas.",
      print: {
        arquivo: "funcionalidades/cancelamento/cancelar-fatura/manual/passo-01-abcd1234.png",
        legenda: "Tela de cancelamento",
        texto_alternativo: "Lista de faturas abertas",
      },
    },
    {
      numero: 2,
      titulo: "Informe o motivo",
      instrucao: "Digite o motivo e clique em **Confirmar**.",
      tela: "Cancelamento de fatura",
    },
  ];
}

function api(): ApiManual {
  return {
    anexarPrint: vi.fn().mockResolvedValue(undefined),
    removerPrint: vi.fn().mockResolvedValue(undefined),
    removerPasso: vi.fn().mockResolvedValue(undefined),
    lerPrint: vi.fn().mockResolvedValue(PNG.buffer),
  };
}

function revisao(): Revisao {
  return { ocupado: false, aoDecidir: vi.fn().mockResolvedValue(undefined) };
}

beforeAll(() => {
  // jsdom não cria URLs de blob.
  URL.createObjectURL = vi.fn(() => "blob:print");
  URL.revokeObjectURL = vi.fn();
});

describe("ManualAoVivo", () => {
  it("mostra o print, a legenda e o marcador do passo sem print", async () => {
    const a = api();
    render(<ManualAoVivo projeto="fat" analise="A-0031" passos={passos()} api={a} />);
    const secao = screen.getByRole("region", { name: "Passo a passo" });
    expect(secao).toHaveTextContent("1 passo sem print");
    const p1 = within(secao).getByRole("listitem", { name: "Passo 1" });
    expect(await within(p1).findByRole("img", { name: "Lista de faturas abertas" })).toHaveAttribute(
      "src",
      "blob:print",
    );
    expect(p1).toHaveTextContent("Figura 1. Tela de cancelamento");
    expect(within(p1).getByText("Faturas").tagName).toBe("STRONG");
    expect(a.lerPrint).toHaveBeenCalledWith(
      "fat",
      "funcionalidades/cancelamento/cancelar-fatura/manual/passo-01-abcd1234.png",
    );
    const p2 = within(secao).getByRole("listitem", { name: "Passo 2" });
    expect(p2).toHaveTextContent("Print sugerido: tela Cancelamento de fatura");
    expect(within(p2).getByText("sem print")).toBeInTheDocument();
    // Sem revisão (aprovado), só leitura.
    expect(within(secao).queryByRole("group", { name: /Revisão do passo/ })).toBeNull();
  });

  it("inclui o print com legenda, texto alternativo e o aviso de dados sensíveis", async () => {
    const a = api();
    render(
      <ManualAoVivo projeto="fat" analise="A-0031" passos={passos()} revisao={revisao()} api={a} />,
    );
    const p2 = screen.getByRole("listitem", { name: "Passo 2" });
    fireEvent.click(within(p2).getByRole("button", { name: "Incluir print" }));
    const form = within(p2).getByRole("form", { name: "Print do passo 2" });
    expect(within(form).getByRole("note")).toHaveTextContent("dados pessoais");
    const salvar = within(form).getByRole("button", { name: "Salvar o print" });
    expect(salvar).toBeDisabled();
    expect(within(form).getByLabelText("Legenda")).toHaveValue("Tela Cancelamento de fatura");
    const arquivo = new File([PNG], "tela.png", { type: "image/png" });
    fireEvent.change(within(form).getByLabelText("Arquivo da imagem"), {
      target: { files: [arquivo] },
    });
    await waitFor(() => expect(salvar).toBeEnabled());
    fireEvent.change(within(form).getByLabelText("Texto alternativo"), {
      target: { value: "Campo do motivo preenchido" },
    });
    fireEvent.click(salvar);
    await waitFor(() =>
      expect(a.anexarPrint).toHaveBeenCalledWith(
        "fat",
        "A-0031",
        2,
        PNG,
        "Tela Cancelamento de fatura",
        "Campo do motivo preenchido",
      ),
    );
    await waitFor(() => expect(within(p2).queryByRole("form")).toBeNull());
  });

  it("dispensa o print, edita a legenda e remove o print e o passo", async () => {
    const a = api();
    const r = revisao();
    render(<ManualAoVivo projeto="fat" analise="A-0031" passos={passos()} revisao={r} api={a} />);
    const p2 = screen.getByRole("listitem", { name: "Passo 2" });
    fireEvent.click(within(p2).getByRole("button", { name: "Dispensar o print" }));
    await waitFor(() =>
      expect(r.aoDecidir).toHaveBeenCalledWith({
        tipo: "dispensar-print",
        numero: 2,
        dispensado: true,
      }),
    );
    const p1 = screen.getByRole("listitem", { name: "Passo 1" });
    fireEvent.click(within(p1).getByRole("button", { name: "Editar legenda" }));
    const form = within(p1).getByRole("form", { name: "Legenda do print do passo 1" });
    fireEvent.change(within(form).getByLabelText("Legenda"), { target: { value: "Faturas abertas" } });
    fireEvent.click(within(form).getByRole("button", { name: "Salvar" }));
    await waitFor(() =>
      expect(r.aoDecidir).toHaveBeenCalledWith({
        tipo: "legenda-print",
        numero: 1,
        legenda: "Faturas abertas",
        texto_alternativo: "Lista de faturas abertas",
      }),
    );
    await waitFor(() => expect(within(p1).queryByRole("form")).toBeNull());
    fireEvent.click(within(p1).getByRole("button", { name: "Remover o print" }));
    await waitFor(() => expect(a.removerPrint).toHaveBeenCalledWith("fat", "A-0031", 1));
    fireEvent.click(within(p1).getByRole("button", { name: "Remover o passo" }));
    await waitFor(() => expect(a.removerPasso).toHaveBeenCalledWith("fat", "A-0031", 1));
  });

  it("edita o texto do passo", async () => {
    const r = revisao();
    render(<ManualAoVivo projeto="fat" analise="A-0031" passos={passos()} revisao={r} api={api()} />);
    const p2 = screen.getByRole("listitem", { name: "Passo 2" });
    fireEvent.click(within(p2).getByRole("button", { name: "Editar o passo" }));
    const form = within(p2).getByRole("form", { name: "Editar o passo 2" });
    fireEvent.change(within(form).getByLabelText("Resultado"), {
      target: { value: "A fatura fica cancelada." },
    });
    fireEvent.click(within(form).getByRole("button", { name: "Salvar" }));
    await waitFor(() =>
      expect(r.aoDecidir).toHaveBeenCalledWith({
        tipo: "editar-passo",
        numero: 2,
        titulo: "Informe o motivo",
        instrucao: "Digite o motivo e clique em **Confirmar**.",
        tela: "Cancelamento de fatura",
        resultado: "A fatura fica cancelada.",
      }),
    );
  });
});

describe("manual", () => {
  it("aceita PNG como está e recusa o que não é imagem", async () => {
    expect(await comoPng(new Blob([PNG], { type: "image/png" }))).toEqual(PNG);
    await expect(comoPng(new Blob(["oi"], { type: "text/plain" }))).rejects.toThrow("imagem");
  });

  it("resolve a imagem do documento na base", () => {
    expect(
      resolverImagem(
        "fat/funcionalidades/cancelamento/cancelar-fatura/manual.md",
        "manual/passo-01.png",
      ),
    ).toBe("fat/funcionalidades/cancelamento/cancelar-fatura/manual/passo-01.png");
    expect(resolverImagem("fat/analises/A-0031.md", "../funcionalidades/c/c/manual/p.png")).toBe(
      "fat/funcionalidades/c/c/manual/p.png",
    );
    expect(resolverImagem("fat/analises/A-0031.md", "https://x.com/a.png")).toBeNull();
    expect(resolverImagem("fat/analises/A-0031.md", "../../../fora.png")).toBeNull();
    expect(resolverImagem("fat/analises/A-0031.md", "a.jpg")).toBeNull();
  });
});
