import { act } from "react";
import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import type { AvisoAnalise } from "./analise";
import { AtaAoVivo, montarConfirmacoes } from "./AtaAoVivo";
import { PainelAnalise } from "./PainelAnalise";
import {
  apiAnaliseFalsa,
  apiConversaFalsa,
  apiTerminalFalsa,
  telaFalsa,
  vistaFicticia,
} from "./test/falsos";
import { DECISAO, apiReuniaoFalsa, ataFicticia } from "./test/reunioesFalso";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(), Channel: vi.fn() }));
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn(), save: vi.fn() }));
vi.mock("./telaXterm", () => ({ criarTelaXterm: vi.fn() }));

function montar() {
  const api = apiReuniaoFalsa();
  const avisos: ((a: AvisoAnalise) => void)[] = [];
  const parar = vi.fn();
  const vigiar = vi.fn((_p: string, f: (a: AvisoAnalise) => void) => {
    avisos.push(f);
    return Promise.resolve(parar);
  });
  const r = render(
    <AtaAoVivo api={api} vigiar={vigiar} projeto="faturamento-exemplo" reuniao="R-0001" />,
  );
  const avisar = (a: AvisoAnalise) => avisos.forEach((f) => f(a));
  return { api, vigiar, avisar, parar, ...r };
}

describe("AtaAoVivo", () => {
  it("mostra os participantes com o papel do catálogo e o proposto pelo agente", async () => {
    montar();
    const maria = await screen.findByTestId("participante-Silva, Maria");
    expect(maria).toHaveTextContent("Catálogo: Maria Silva, PO (confirmado em D-0001/R-0000)");
    expect(within(maria).getByText("papel a confirmar")).toBeInTheDocument();
    // O papel do catálogo vem preenchido para confirmar.
    expect(within(maria).getByLabelText("Papel de Silva, Maria")).toHaveValue("PO");

    const joao = screen.getByTestId("participante-João Pereira");
    expect(joao).toHaveTextContent("Catálogo: pessoa nova");
    expect(joao).toHaveTextContent("Agente propõe: Desenvolvedor");
    expect(joao).toHaveTextContent("Explica onde fica a trava no código.");
    expect(within(joao).getByRole("list", { name: "Falas de origem" })).toHaveTextContent(
      "00:00:30 João Pereira: “a trava dos 30 dias só existe na tela de retaguarda” (conferida)",
    );
    expect(within(joao).getByLabelText("Papel de João Pereira")).toHaveValue("Desenvolvedor");
    expect(screen.getByText(/Transcript guardado sem aprovação, apagado em 2026-12-14/)).toBeVisible();
  });

  it("se atualiza a cada registrar_item_ata (aviso de ata)", async () => {
    const { api, vigiar, avisar, parar, unmount } = montar();
    expect(await screen.findByText(/Nenhum item registrado ainda/)).toBeInTheDocument();
    expect(vigiar).toHaveBeenCalledWith("faturamento-exemplo", expect.any(Function));
    expect(api.ataReuniao).toHaveBeenCalledTimes(1);

    // Aviso de outra reunião ou de uma análise não relê.
    act(() => avisar({ tipo: "mudanca", analise: "R-0002", mudanca: "ata", versao: 0 }));
    act(() => avisar({ tipo: "mudanca", analise: "A-0001", mudanca: "afirmacao", versao: 3 }));
    expect(api.ataReuniao).toHaveBeenCalledTimes(1);

    api.ataReuniao.mockResolvedValue(ataFicticia({ itens: [DECISAO] }));
    act(() => avisar({ tipo: "mudanca", analise: "R-0001", mudanca: "ata", versao: 0 }));
    const item = await screen.findByTestId("item-D-1");
    expect(item).toHaveTextContent("A trava dos 30 dias vale também no banco");
    expect(item).toHaveTextContent("00:01:40 Silva, Maria: “vamos levar a trava para o banco”");
    expect(within(item).getByText("db/03_pkg_cobranca.sql:71-72")).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "Decisões" })).toBeInTheDocument();
    expect(screen.getByText(/1 item\(ns\) registrado/)).toBeInTheDocument();
    expect(api.ataReuniao).toHaveBeenCalledTimes(2);

    unmount();
    expect(parar).toHaveBeenCalled();
  });

  it("confirma os papéis e junta dois nomes que são a mesma pessoa", async () => {
    const { api } = montar();
    const joao = await screen.findByTestId("participante-João Pereira");
    fireEvent.change(within(joao).getByLabelText("Papel de João Pereira"), {
      target: { value: "Arquiteto do legado" },
    });
    const outra = screen.getByTestId("participante-Maria S");
    fireEvent.change(within(outra).getByLabelText("É a mesma pessoa que (Maria S)"), {
      target: { value: "Silva, Maria" },
    });
    expect(within(outra).getByLabelText("Papel de Maria S")).toBeDisabled();

    api.confirmarParticipantes.mockResolvedValueOnce(
      ataFicticia({
        participantes: [
          { ...ataFicticia().participantes[0], papel: "PO" },
          { ...ataFicticia().participantes[1], papel: "Arquiteto do legado" },
        ],
      }),
    );
    fireEvent.click(screen.getByRole("button", { name: "Confirmar os participantes" }));
    await waitFor(() =>
      expect(api.confirmarParticipantes).toHaveBeenCalledWith("faturamento-exemplo", "R-0001", [
        { nome: "Silva, Maria", papel: "PO", pessoa: "maria-silva", mesmos: ["Maria S"] },
        { nome: "João Pereira", papel: "Arquiteto do legado", pessoa: null, mesmos: [] },
      ]),
    );
    expect(await screen.findByRole("status")).toHaveTextContent(
      "o catálogo de pessoas foi atualizado",
    );
    expect(screen.queryByTestId("participante-Maria S")).toBeNull();
    expect(
      within(screen.getByTestId("participante-João Pereira")).getByText("Arquiteto do legado", {
        selector: ".badge",
      }),
    ).toBeInTheDocument();
  });

  it("ata aprovada fica só para leitura", async () => {
    const api = apiReuniaoFalsa(ataFicticia({ status: "aprovada", transcript: null }));
    render(
      <AtaAoVivo
        api={api}
        vigiar={() => Promise.resolve(() => undefined)}
        projeto="faturamento-exemplo"
        reuniao="R-0001"
      />,
    );
    expect(await screen.findByText("ata aprovada")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Confirmar os participantes" })).toBeNull();
    expect(screen.queryByLabelText(/Papel de/)).toBeNull();
  });
});

describe("montarConfirmacoes", () => {
  it("segue as junções em cadeia até o participante que fica", () => {
    const ps = ataFicticia().participantes;
    const r = (mesmoQue: string | null, papel = "") => ({ papel, pessoa: null, mesmoQue });
    const c = montarConfirmacoes(ps, {
      "Silva, Maria": r(null, " PO "),
      "João Pereira": r("Maria S"),
      "Maria S": r("Silva, Maria"),
    });
    expect(c).toEqual([
      { nome: "Silva, Maria", papel: "PO", pessoa: null, mesmos: ["João Pereira", "Maria S"] },
    ]);
  });
});

describe("PainelAnalise de uma reunião", () => {
  it("mostra a ata ao vivo ao lado da conversa", async () => {
    const analise = apiAnaliseFalsa(
      vistaFicticia({}, { fluxo: "reuniao", reuniao: "R-0001", titulo: "Reunião R-0001" }),
    );
    const reuniao = apiReuniaoFalsa();
    render(
      <PainelAnalise
        api={analise.api}
        projeto="faturamento-exemplo"
        analise="A-0001"
        abertura={{ tipo: "reabrir" }}
        aoVoltar={vi.fn()}
        apiTerminal={apiTerminalFalsa().api}
        apiConversa={apiConversaFalsa()}
        apiReuniao={reuniao}
        criarTela={() => telaFalsa()}
      />,
    );
    expect(await screen.findByRole("heading", { name: "Ata da reunião R-0001" })).toBeVisible();
    expect(reuniao.ataReuniao).toHaveBeenCalledWith("faturamento-exemplo", "R-0001");
  });
});
