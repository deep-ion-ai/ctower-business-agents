import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import type { Evidencia, Funcionalidade, PontoJustificado } from "./analise";
import { FuncionalidadesAoVivo } from "./FuncionalidadesAoVivo";
import type { Revisao } from "./AcoesRevisao";
import type { Completude } from "./funcionalidades";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(), Channel: vi.fn() }));

function ev(arquivo: string, linhas: string): Evidencia {
  return { arquivo, linhas, trecho: "x", hash: "h", estado: "valida" };
}

/** Mapa fictício do "Faturamento Exemplo". */
function funcionalidades(): Funcionalidade[] {
  return [
    {
      id: "F-CAN-001",
      nome: "Cancelar fatura",
      area: "cancelamento",
      area_nome: "Cancelamento",
      descricao: "Cancela uma fatura aberta, com motivo.",
      atores: ["Atendente da retaguarda"],
      pontos: [
        {
          chave: "delphi/UFrmCancelamento.dfm#FrmCancelamento",
          tipo: "tela",
          nome: "FrmCancelamento",
          evidencia: ev("delphi/UFrmCancelamento.dfm", "1"),
        },
        {
          chave: "db/03_pkg_cobranca.sql#PKG_COBRANCA.PRC_CANCELAR_FATURA",
          tipo: "rotina_banco",
          nome: "PKG_COBRANCA.PRC_CANCELAR_FATURA",
          evidencia: ev("db/03_pkg_cobranca.sql", "99"),
        },
      ],
      dados_gravados: ["FATURA", "LOG_FATURA"],
      situacao: "pendente",
      do_catalogo: true,
      registrada_ms: 1,
    },
    {
      id: "F-COB-001",
      nome: "Calcular encargos por atraso",
      area: "cobranca",
      area_nome: "Cobrança",
      descricao: "Calcula multa e juros.",
      pontos: [
        {
          nome: "menu montado por tabela",
          evidencia: ev("db/02_parametros.sql", "6-7"),
        },
      ],
      situacao: "aceita",
      registrada_ms: 2,
    },
  ];
}

const justificados: PontoJustificado[] = [
  {
    chave: "db/03_pkg_cobranca.sql#PKG_COBRANCA.FN_PARAMETRO",
    tipo: "rotina_banco",
    nome: "PKG_COBRANCA.FN_PARAMETRO",
    arquivo: "db/03_pkg_cobranca.sql",
    motivo: "utilitario",
    justificativa: "Lê os parâmetros.",
    registrado_ms: 3,
  },
];

function revisao() {
  const aoDecidir = vi.fn().mockResolvedValue(undefined);
  const r: Revisao = { aoDecidir, ocupado: false };
  return { r, aoDecidir };
}

describe("FuncionalidadesAoVivo", () => {
  it("agrupa por área e mostra pontos, justificados e sem tratamento", async () => {
    const completude: Completude = {
      total: 15,
      semTratamento: [
        {
          chave: "db/04_pkg_situacao.sql#PKG_SITUACAO.PRC_SUSPENDER_COBRANCA",
          tipo: "rotina_banco",
          nome: "PKG_SITUACAO.PRC_SUSPENDER_COBRANCA",
          arquivo: "db/04_pkg_situacao.sql",
          linha: 113,
        },
      ],
    };
    render(
      <FuncionalidadesAoVivo
        funcionalidades={funcionalidades()}
        justificados={justificados}
        escopo={{ pasta: "db" }}
        carregarCompletude={() => Promise.resolve(completude)}
      />,
    );
    expect(screen.getByRole("heading", { name: "Cancelamento" })).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "Cobrança" })).toBeInTheDocument();
    const can = screen.getByRole("region", { name: "F-CAN-001: Cancelar fatura" });
    expect(within(can).getByText("já no catálogo")).toBeInTheDocument();
    expect(within(can).getByText("FrmCancelamento")).toBeInTheDocument();
    expect(screen.getByText("fora da lista do mapa")).toBeInTheDocument();
    expect(screen.getByText(/a pasta db/)).toBeInTheDocument();
    const sem = await screen.findByRole("list", { name: "Sem tratamento" });
    expect(within(sem).getByText("PKG_SITUACAO.PRC_SUSPENDER_COBRANCA")).toBeInTheDocument();
    expect(screen.getByText(/1 sem tratamento/)).toBeInTheDocument();
    const just = screen.getByRole("list", { name: "Justificados" });
    expect(within(just).getByText(/utilitário/)).toBeInTheDocument();
    // Sem revisão (análise aprovada), só leitura.
    expect(screen.queryByRole("button", { name: "Aceitar" })).not.toBeInTheDocument();
  });

  it("aceita, edita, junta e divide", async () => {
    const { r, aoDecidir } = revisao();
    render(
      <FuncionalidadesAoVivo
        funcionalidades={funcionalidades()}
        justificados={[]}
        revisao={r}
      />,
    );
    const can = screen.getByRole("group", { name: "Revisão de F-CAN-001" });
    fireEvent.click(within(can).getByRole("button", { name: "Aceitar" }));
    expect(aoDecidir).toHaveBeenCalledWith({
      tipo: "funcionalidade",
      funcionalidade: "F-CAN-001",
      decisao: "aceitar",
    });

    fireEvent.click(within(can).getByRole("button", { name: "Editar" }));
    const editar = screen.getByRole("form", { name: "Editar F-CAN-001" });
    fireEvent.change(within(editar).getByLabelText("Nome"), {
      target: { value: "Cancelar uma fatura" },
    });
    fireEvent.click(within(editar).getByRole("button", { name: "Salvar" }));
    await waitFor(() =>
      expect(aoDecidir).toHaveBeenCalledWith({
        tipo: "editar-funcionalidade",
        funcionalidade: "F-CAN-001",
        nome: "Cancelar uma fatura",
        area: "Cancelamento",
        descricao: "Cancela uma fatura aberta, com motivo.",
      }),
    );

    await waitFor(() =>
      expect(screen.getByRole("group", { name: "Revisão de F-CAN-001" })).toBeInTheDocument(),
    );
    fireEvent.click(
      within(screen.getByRole("group", { name: "Revisão de F-CAN-001" })).getByRole("button", {
        name: "Juntar",
      }),
    );
    const juntar = screen.getByRole("form", { name: "Juntar F-CAN-001" });
    fireEvent.click(within(juntar).getByRole("button", { name: "Juntar" }));
    await waitFor(() =>
      expect(aoDecidir).toHaveBeenCalledWith({
        tipo: "juntar-funcionalidades",
        origem: "F-CAN-001",
        destino: "F-COB-001",
      }),
    );

    await waitFor(() =>
      expect(screen.getByRole("group", { name: "Revisão de F-CAN-001" })).toBeInTheDocument(),
    );
    fireEvent.click(
      within(screen.getByRole("group", { name: "Revisão de F-CAN-001" })).getByRole("button", {
        name: "Dividir",
      }),
    );
    const dividir = screen.getByRole("form", { name: "Dividir F-CAN-001" });
    const enviar = within(dividir).getByRole("button", { name: "Dividir" });
    expect(enviar).toBeDisabled();
    fireEvent.click(within(dividir).getByLabelText("PKG_COBRANCA.PRC_CANCELAR_FATURA"));
    fireEvent.change(within(dividir).getByLabelText("Nome da nova"), {
      target: { value: "Validar cancelamento no banco" },
    });
    fireEvent.change(within(dividir).getByLabelText("Descrição da nova"), {
      target: { value: "Confere a situação e grava o histórico." },
    });
    fireEvent.click(enviar);
    await waitFor(() =>
      expect(aoDecidir).toHaveBeenCalledWith({
        tipo: "dividir-funcionalidade",
        funcionalidade: "F-CAN-001",
        pontos: [1],
        nome: "Validar cancelamento no banco",
        descricao: "Confere a situação e grava o histórico.",
      }),
    );
    // A aceita não tem "Aceitar"; tem "Desfazer" e "Descartar".
    const cob = screen.getByRole("group", { name: "Revisão de F-COB-001" });
    expect(within(cob).queryByRole("button", { name: "Aceitar" })).not.toBeInTheDocument();
    fireEvent.click(within(cob).getByRole("button", { name: "Descartar" }));
    expect(aoDecidir).toHaveBeenLastCalledWith({
      tipo: "funcionalidade",
      funcionalidade: "F-COB-001",
      decisao: "descartar",
    });
  });
});
