import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { PainelBancoAnalise } from "./PainelBancoAnalise";
import type { ApiConsultasBanco, ConsultaDados } from "./consultasBanco";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(), Channel: vi.fn() }));

const PROJETO = "faturamento-exemplo";

function apiFalsa(bancos = true): ApiConsultasBanco {
  return {
    bancosProjeto: vi.fn().mockResolvedValue(
      bancos
        ? [
            {
              id: "banco-hml",
              nome: "Banco de homologação",
              rotuloTipo: "PostgreSQL",
              ambiente: "homologação",
              cadastrada: true,
            },
          ]
        : [],
    ),
    salvarBancosProjeto: vi.fn(),
    ligarBancoAnalise: vi.fn().mockResolvedValue(undefined),
    decidirConsulta: vi.fn().mockResolvedValue(undefined),
  };
}

const PENDENTE: ConsultaDados = {
  numero: 3,
  objetivo: "medir quantas faturas estão na mesma situação",
  origem: {
    conexao: "banco-hml",
    nome: "Banco de homologação",
    tipo: "PostgreSQL",
    ambiente: "homologação",
  },
  sql_proposto: "SELECT situacao, count(*)\nFROM fatura\nGROUP BY situacao",
  situacao: "pendente",
  pedida_ms: 1_795_000_000_000,
};

const RECUSADA: ConsultaDados = {
  ...PENDENTE,
  numero: 2,
  objetivo: "ver clientes",
  sql_proposto: "SELECT * FROM cliente",
  situacao: "recusada",
  decidida_por: "ana",
  decidida_ms: 1_795_000_000_000,
  motivo: "traz clientes inteiros",
};

function montar(api: ApiConsultasBanco, consultas: ConsultaDados[] = [], ligado = false) {
  render(
    <PainelBancoAnalise
      api={api}
      projeto={PROJETO}
      analise="A-0007"
      aprovada={false}
      banco={
        ligado
          ? { conexao: "banco-hml", nome: "Banco de homologação", ligado_ms: 1 }
          : null
      }
      consultas={consultas}
    />,
  );
}

describe("PainelBancoAnalise", () => {
  it("sem banco no projeto, não mostra nada", async () => {
    const api = apiFalsa(false);
    montar(api);
    await waitFor(() => expect(api.bancosProjeto).toHaveBeenCalled());
    expect(screen.queryByText(/Usar o banco/)).toBeNull();
  });

  it("liga e desliga o banco na análise", async () => {
    const api = apiFalsa();
    montar(api);
    const caixa = await screen.findByRole("checkbox", {
      name: /Usar o banco Banco de homologação nesta análise/,
    });
    expect(caixa).not.toBeChecked();
    fireEvent.click(caixa);
    await waitFor(() =>
      expect(api.ligarBancoAnalise).toHaveBeenCalledWith(PROJETO, "A-0007", "banco-hml"),
    );
  });

  it("mostra a consulta aguardando aprovação e executa", async () => {
    const api = apiFalsa();
    montar(api, [PENDENTE], true);
    const pedido = await screen.findByRole("group", { name: "Consulta 3" });
    expect(within(pedido).getByText("aguardando sua aprovação")).toBeInTheDocument();
    expect(within(pedido).getByText(/medir quantas faturas/)).toBeInTheDocument();
    expect(
      within(pedido).getByText(/Banco de homologação \(PostgreSQL, homologação\)/),
    ).toBeInTheDocument();
    expect(pedido.querySelector("pre")?.textContent).toContain("GROUP BY situacao");
    fireEvent.click(within(pedido).getByRole("button", { name: "Executar" }));
    await waitFor(() =>
      expect(api.decidirConsulta).toHaveBeenCalledWith(PROJETO, "A-0007", 3, {
        decisao: "executar",
      }),
    );
  });

  it("edita o SQL antes de executar", async () => {
    const api = apiFalsa();
    montar(api, [PENDENTE], true);
    const pedido = await screen.findByRole("group", { name: "Consulta 3" });
    fireEvent.click(within(pedido).getByRole("button", { name: "Editar e executar" }));
    fireEvent.change(within(pedido).getByLabelText("SQL a executar"), {
      target: { value: "SELECT count(*) FROM fatura" },
    });
    fireEvent.click(within(pedido).getByRole("button", { name: "Executar o SQL editado" }));
    await waitFor(() =>
      expect(api.decidirConsulta).toHaveBeenCalledWith(PROJETO, "A-0007", 3, {
        decisao: "editar_e_executar",
        sql: "SELECT count(*) FROM fatura",
      }),
    );
  });

  it("recusa só com motivo, que vai para o agente", async () => {
    const api = apiFalsa();
    montar(api, [PENDENTE], true);
    const pedido = await screen.findByRole("group", { name: "Consulta 3" });
    fireEvent.click(within(pedido).getByRole("button", { name: "Recusar" }));
    const recusar = within(pedido).getByRole("button", { name: "Recusar consulta" });
    expect(recusar).toBeDisabled();
    fireEvent.change(within(pedido).getByLabelText(/Motivo/), {
      target: { value: "  use COUNT por situação " },
    });
    fireEvent.click(recusar);
    await waitFor(() =>
      expect(api.decidirConsulta).toHaveBeenCalledWith(PROJETO, "A-0007", 3, {
        decisao: "recusar",
        motivo: "use COUNT por situação",
      }),
    );
  });

  it("registra todas as consultas, com quem decidiu e o motivo", async () => {
    const api = apiFalsa();
    montar(api, [RECUSADA, PENDENTE], true);
    const registro = await screen.findByText(/Consultas ao banco nesta análise \(2\)/);
    const lista = registro.closest("details") as HTMLElement;
    expect(within(lista).getByText(/Consulta 2/).parentElement?.textContent).toContain(
      "recusada por ana",
    );
    expect(within(lista).getByText("Motivo: traz clientes inteiros")).toBeInTheDocument();
  });
});
