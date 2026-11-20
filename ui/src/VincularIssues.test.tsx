import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import type { VistaDeIssues } from "./funcionalidades";
import type { ApiTerminal } from "./terminal";
import { apiFalsa } from "./test/mapaFalso";
import { VincularIssues } from "./VincularIssues";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(), Channel: vi.fn() }));
vi.mock("./telaXterm", () => ({ criarTelaXterm: vi.fn() }));

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
        { funcionalidade: "F-FAT-001", nome: "Parcelar fatura", pontuacao: 9, evidencias: ["parcel… (nome)"], ia: false },
      ],
      demanda: null,
      ligadaA: [],
      justificativa: null,
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
      justificativa: null,
    },
  ],
};

const telaFalsa = () => ({
  abrirEm: vi.fn(),
  escrever: vi.fn(),
  aoDigitar: vi.fn(),
  ajustar: vi.fn(() => ({ linhas: 24, colunas: 80 })),
  limpar: vi.fn(),
  focar: vi.fn(),
  descartar: vi.fn(),
});
const apiTerminal = {
  abrir: vi.fn().mockResolvedValue(7),
  escrever: vi.fn().mockResolvedValue(undefined),
  redimensionar: vi.fn().mockResolvedValue(undefined),
  encerrar: vi.fn().mockResolvedValue(undefined),
  enviarPrompt: vi.fn().mockResolvedValue(undefined),
} as unknown as ApiTerminal;

function abrir() {
  const api = apiFalsa();
  (api.issues as ReturnType<typeof vi.fn>).mockResolvedValue(VISTA);
  const aoVincular = vi.fn();
  render(
    <VincularIssues
      projeto="p"
      api={api}
      aoVoltar={vi.fn()}
      aoVincular={aoVincular}
      apiTerminal={apiTerminal}
      criarTela={() => telaFalsa() as never}
    />,
  );
  return { api, aoVincular };
}

describe("VincularIssues", () => {
  it("lê 6 meses por padrão e mostra sugestão, pontuação e issue sem funcionalidade", async () => {
    const { api } = abrir();
    expect(screen.getByLabelText("Meses")).toHaveValue(6);
    fireEvent.click(screen.getByRole("button", { name: "Buscar issues" }));
    expect(await screen.findByLabelText("Resumo da leitura")).toHaveTextContent(
      "2 issue(s) criadas desde 2026-05-19 · 0 confirmada(s) · 1 com sugestão · 1 sem funcionalidade",
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
    (api.vincular as ReturnType<typeof vi.fn>).mockResolvedValue({
      demandas: [{ origem: "github", chave: "#41", demanda: "D-0001" }],
      criadas: ["D-0001"],
      reaproveitadas: [],
      vinculosNovos: 1,
      vinculosExistentes: 0,
      avisos: [],
    });
    fireEvent.click(screen.getByRole("button", { name: "Confirmar 1 issue" }));
    await waitFor(() => expect(api.vincular).toHaveBeenCalledTimes(1));
    const [, itens] = (api.vincular as ReturnType<typeof vi.fn>).mock.calls[0];
    expect(itens).toHaveLength(1);
    expect(itens[0].funcionalidades).toEqual(["F-FAT-001"]);
    expect(itens[0].issue.chave).toBe("#41");
    expect(itens[0].issue.sugestoes).toBeUndefined();
    await waitFor(() => expect(aoVincular).toHaveBeenCalled());
    expect(await screen.findByRole("status")).toHaveTextContent("1 vínculo(s) novo(s)");
    // Marca a issue como confirmada sem reler o rastreador.
    expect(api.issues).toHaveBeenCalledTimes(1);
    expect(await screen.findByText("✓ Confirmada")).toBeInTheDocument();
  });

  it("troca a funcionalidade, descarta e desfaz", async () => {
    const { api } = abrir();
    fireEvent.click(screen.getByRole("button", { name: "Buscar issues" }));
    await screen.findByLabelText("Resumo da leitura");
    // Issue sem sugestão: escolhe outra funcionalidade e confirma só ela.
    fireEvent.click(screen.getAllByRole("button", { name: "Outra funcionalidade…" })[1]);
    fireEvent.click(within(screen.getByRole("dialog")).getByRole("checkbox", { name: /F-FAT-002/ }));
    fireEvent.click(screen.getByRole("button", { name: "Adicionar 1" }));
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

  it("marca como confirmada a issue já ligada e a tira do lote", async () => {
    const api = apiFalsa();
    const [a, b] = VISTA.issues;
    (api.issues as ReturnType<typeof vi.fn>).mockResolvedValue({
      ...VISTA,
      issues: [{ ...a, demanda: "D-0001", ligadaA: ["F-FAT-001"] }, b],
    });
    render(<VincularIssues projeto="p" api={api} aoVoltar={vi.fn()} aoVincular={vi.fn()} />);
    fireEvent.click(screen.getByRole("button", { name: "Buscar issues" }));
    expect(await screen.findByLabelText("Resumo da leitura")).toHaveTextContent(
      "1 confirmada(s) · 0 com sugestão · 1 sem funcionalidade",
    );
    expect(screen.getByText("✓ Confirmada")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Confirmar 0 issues" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Confirmar as novas" })).toBeDisabled();
    // Acrescentar outra funcionalidade libera só as novas.
    fireEvent.click(screen.getAllByRole("button", { name: "Outra funcionalidade…" })[0]);
    const modal = within(screen.getByRole("dialog", { name: "Funcionalidades de #41" }));
    expect(modal.getByText("Parcelamento erra as parcelas")).toBeInTheDocument();
    // A já ligada aparece marcada e travada.
    expect(modal.getByRole("checkbox", { name: /F-FAT-001/ })).toBeDisabled();
    fireEvent.click(modal.getByRole("checkbox", { name: /F-FAT-002/ }));
    fireEvent.click(screen.getByRole("button", { name: "Adicionar 1" }));
    expect(screen.getByRole("button", { name: "Confirmar as novas" })).toBeEnabled();
    expect(screen.getByRole("button", { name: "Confirmar 1 issue" })).toBeInTheDocument();
    // Filtros por situação.
    fireEvent.change(screen.getByLabelText(/Vínculo/), { target: { value: "confirmadas" } });
    const lista = screen.getByRole("list", { name: "Issues" });
    expect(within(lista).getByText("Parcelamento erra as parcelas")).toBeInTheDocument();
    expect(within(lista).queryByText("Trocar a cor do botão")).toBeNull();
    fireEvent.change(screen.getByLabelText(/Vínculo/), { target: { value: "pendentes" } });
    expect(within(lista).queryByText("Parcelamento erra as parcelas")).toBeNull();
  });

  it("justifica a issue sem funcionalidade e desfaz", async () => {
    const { api } = abrir();
    fireEvent.click(screen.getByRole("button", { name: "Buscar issues" }));
    await screen.findByLabelText("Resumo da leitura");
    // Só a #41 tem sugestão, mas as duas aceitam justificativa: usa a #42.
    const botoes = screen.getAllByRole("button", { name: "Justificar sem funcionalidade" });
    fireEvent.click(botoes[botoes.length - 1]);
    const salvar = screen.getByRole("button", { name: "Salvar justificativa" });
    expect(salvar).toBeDisabled();
    fireEvent.change(screen.getByLabelText("Motivo de #42"), { target: { value: "infraestrutura" } });
    fireEvent.change(screen.getByLabelText("Justificativa de #42"), { target: { value: "Só visual" } });
    fireEvent.click(salvar);
    expect(await screen.findByText(/infraestrutura: Só visual/)).toBeInTheDocument();
    expect(api.justificar).toHaveBeenCalledWith(
      "p",
      expect.objectContaining({ chave: "#42" }),
      { motivo: "infraestrutura", texto: "Só visual" },
    );
    fireEvent.click(screen.getByRole("button", { name: "Desfazer justificativa" }));
    await waitFor(() => expect(screen.queryByText(/Só visual/)).toBeNull());
    expect(api.justificar).toHaveBeenLastCalledWith("p", expect.anything(), null);
  });

  it("sugere com IA as issues sem sugestão e mostra a proposta do agente ao reler", async () => {
    const { api } = abrir();
    fireEvent.click(screen.getByRole("button", { name: "Buscar issues" }));
    await screen.findByLabelText("Resumo da leitura");
    fireEvent.click(screen.getByRole("button", { name: "Sugerir com IA (1)" }));
    expect(await screen.findByRole("region", { name: "Sugestão com IA" })).toBeInTheDocument();
    const enviadas = (api.pedidoVinculoIa as ReturnType<typeof vi.fn>).mock.calls[0][1];
    expect(enviadas.map((x: { chave: string }) => x.chave)).toEqual(["#42"]);
    // O agente propôs: a releitura traz a sugestão da IA com a evidência.
    const comProposta = structuredClone(VISTA);
    comProposta.issues[1].sugestoes = [
      { funcionalidade: "F-FAT-002", nome: "Cancelar nota", pontuacao: 0, evidencias: ["IA: botão de cancelar"], ia: true },
    ];
    (api.issues as ReturnType<typeof vi.fn>).mockResolvedValue(comProposta);
    fireEvent.click(screen.getByRole("button", { name: "Encerrar a sessão e ver as propostas" }));
    expect(await screen.findByLabelText("Funcionalidades de #42")).toHaveTextContent("IA");
    expect(screen.getByLabelText("Funcionalidades de #42")).toHaveTextContent("botão de cancelar");
    expect(screen.getByRole("checkbox", { name: /F-FAT-002/ })).toBeChecked();
  });

  it("modal: busca, escolhe mais de uma funcionalidade e adiciona de uma vez", async () => {
    abrir();
    fireEvent.click(screen.getByRole("button", { name: "Buscar issues" }));
    await screen.findByLabelText("Resumo da leitura");
    fireEvent.click(screen.getAllByRole("button", { name: "Outra funcionalidade…" })[1]);
    const modal = within(screen.getByRole("dialog"));
    expect(modal.getByText(/Cobrança/)).toBeInTheDocument();
    // A busca filtra por nome.
    fireEvent.change(modal.getByLabelText("Buscar funcionalidade"), { target: { value: "cancel" } });
    expect(modal.queryByRole("checkbox", { name: /F-FAT-001/ })).toBeNull();
    fireEvent.click(modal.getByRole("checkbox", { name: /F-FAT-002/ }));
    fireEvent.change(modal.getByLabelText("Buscar funcionalidade"), { target: { value: "" } });
    fireEvent.click(modal.getByRole("checkbox", { name: /F-FAT-001/ }));
    fireEvent.click(modal.getByRole("button", { name: "Adicionar 2" }));
    expect(screen.queryByRole("dialog")).toBeNull();
    const lista = screen.getByLabelText("Funcionalidades de #42");
    expect(within(lista).getByRole("checkbox", { name: /F-FAT-001/ })).toBeChecked();
    expect(within(lista).getByRole("checkbox", { name: /F-FAT-002/ })).toBeChecked();
  });
});
