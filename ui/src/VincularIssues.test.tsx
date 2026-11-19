import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import type { VistaDeIssues } from "./funcionalidades";
import { apiFalsa } from "./test/mapaFalso";
import { VincularIssues } from "./VincularIssues";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(), Channel: vi.fn() }));

const VISTA: VistaDeIssues = {
  desde: "2026-05-19",
  excedeuTeto: true,
  funcionalidades: [
    { id: "F-FAT-001", nome: "Parcelar fatura", areaNome: "Cobrança" },
    { id: "F-FAT-002", nome: "Cancelar nota", areaNome: "Cobrança" },
  ],
  issues: [
    {
      origem: "github",
      chave: "#41",
      titulo: "Parcelamento erra as parcelas",
      descricao: "d",
      url: "https://github.com/o/r/issues/41",
      criadaEm: "2026-10-01",
      estado: "aberta",
      rotulos: ["bug"],
      sugestoes: [
        { funcionalidade: "F-FAT-001", nome: "Parcelar fatura", pontuacao: 9, evidencias: ["parcel… (nome)"] },
      ],
      demanda: null,
      ligadaA: [],
    },
    {
      origem: "github",
      chave: "#42",
      titulo: "Trocar a cor do botão",
      descricao: "",
      url: "https://github.com/o/r/issues/42",
      criadaEm: "2026-10-02",
      estado: "fechada",
      rotulos: [],
      sugestoes: [],
      demanda: null,
      ligadaA: [],
    },
  ],
};

function abrir() {
  const api = apiFalsa();
  (api.issues as ReturnType<typeof vi.fn>).mockResolvedValue(VISTA);
  const aoVincular = vi.fn();
  render(<VincularIssues projeto="p" api={api} aoVoltar={vi.fn()} aoVincular={aoVincular} />);
  return { api, aoVincular };
}

describe("VincularIssues", () => {
  it("lê 6 meses por padrão e mostra sugestão, pontuação e issue sem funcionalidade", async () => {
    const { api } = abrir();
    expect(screen.getByLabelText("Meses")).toHaveValue(6);
    fireEvent.click(screen.getByRole("button", { name: "Buscar issues" }));
    expect(await screen.findByLabelText("Resumo da leitura")).toHaveTextContent(
      "2 issue(s) criadas desde 2026-05-19 · 1 com sugestão · 1 sem funcionalidade",
    );
    expect(api.issues).toHaveBeenCalledWith("p", 6, null);
    expect(screen.getByText(/mais issues que o limite/)).toBeInTheDocument();
    expect(screen.getByLabelText("Funcionalidades de #41")).toHaveTextContent("9 pontos");
    expect(screen.getByRole("checkbox", { name: /F-FAT-001/ })).toBeChecked();
    expect(screen.getAllByText("Sem funcionalidade sugerida.")).toHaveLength(1);
  });

  it("muda o período e filtra por vínculo", async () => {
    const { api } = abrir();
    fireEvent.change(screen.getByLabelText("Meses"), { target: { value: "12" } });
    fireEvent.click(screen.getByRole("button", { name: "Buscar issues" }));
    await screen.findByLabelText("Resumo da leitura");
    expect(api.issues).toHaveBeenCalledWith("p", 12, null);
    fireEvent.change(screen.getByLabelText(/Vínculo/), { target: { value: "sem" } });
    const lista = screen.getByRole("list", { name: "Issues" });
    expect(within(lista).queryByText("Parcelamento erra as parcelas")).toBeNull();
    expect(within(lista).getByText("Trocar a cor do botão")).toBeInTheDocument();
  });

  it("confirma o lote só com as issues que têm funcionalidade e relê depois", async () => {
    const { api, aoVincular } = abrir();
    fireEvent.click(screen.getByRole("button", { name: "Buscar issues" }));
    await screen.findByLabelText("Resumo da leitura");
    fireEvent.click(screen.getByRole("button", { name: "Confirmar 1 issue" }));
    await waitFor(() => expect(api.vincular).toHaveBeenCalledTimes(1));
    const [, itens] = (api.vincular as ReturnType<typeof vi.fn>).mock.calls[0];
    expect(itens).toHaveLength(1);
    expect(itens[0].funcionalidades).toEqual(["F-FAT-001"]);
    expect(itens[0].issue.chave).toBe("#41");
    expect(itens[0].issue.sugestoes).toBeUndefined();
    await waitFor(() => expect(aoVincular).toHaveBeenCalled());
    expect(await screen.findByRole("status")).toHaveTextContent("0 vínculo(s) novo(s)");
    expect(api.issues).toHaveBeenCalledTimes(2);
  });

  it("troca a funcionalidade, descarta e desfaz", async () => {
    const { api } = abrir();
    fireEvent.click(screen.getByRole("button", { name: "Buscar issues" }));
    await screen.findByLabelText("Resumo da leitura");
    // Issue sem sugestão: escolhe outra funcionalidade e confirma só ela.
    fireEvent.change(screen.getByLabelText("Outra funcionalidade para #42"), {
      target: { value: "F-FAT-002" },
    });
    expect(screen.getByRole("button", { name: "Confirmar 2 issues" })).toBeInTheDocument();
    // Descarta a primeira: sai do lote.
    fireEvent.click(screen.getAllByRole("button", { name: "Descartar" })[0]);
    expect(screen.getByText(/Descartada: fica sem funcionalidade/)).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Confirmar 1 issue" }));
    await waitFor(() => expect(api.vincular).toHaveBeenCalled());
    const [, itens] = (api.vincular as ReturnType<typeof vi.fn>).mock.calls[0];
    expect(itens[0].funcionalidades).toEqual(["F-FAT-002"]);
  });

  it("mostra a falha da leitura", async () => {
    const api = apiFalsa();
    (api.issues as ReturnType<typeof vi.fn>).mockRejectedValue("o projeto não tem repositório de issues cadastrado");
    render(<VincularIssues projeto="p" api={api} aoVoltar={vi.fn()} aoVincular={vi.fn()} />);
    fireEvent.click(screen.getByRole("button", { name: "Buscar issues" }));
    expect(await screen.findByText(/repositório de issues/)).toBeInTheDocument();
  });
});
