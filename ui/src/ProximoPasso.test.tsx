import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { AtaAoVivo } from "./AtaAoVivo";
import { PainelDemanda } from "./PainelDemanda";
import { necessidadesDe } from "./ProximoPasso";
import {
  PROJETO,
  apiConhecimentoFalsa,
  demandasFicticias,
  painelFicticio,
} from "./test/conhecimentoFalso";
import {
  NECESSIDADE,
  PROXIMO_PASSO,
  analiseCriadaFicticia,
  apiReuniaoFalsa,
  ataFicticia,
} from "./test/reunioesFalso";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(), Channel: vi.fn() }));
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn(), save: vi.fn() }));

const aprovada = ataFicticia({
  status: "aprovada",
  transcript: null,
  itens: [NECESSIDADE, PROXIMO_PASSO],
});

function montarAta(ata = aprovada) {
  const api = apiReuniaoFalsa(ata);
  const aoCriarAnalise = vi.fn();
  render(
    <AtaAoVivo
      api={api}
      vigiar={() => Promise.resolve(() => undefined)}
      projeto={PROJETO}
      reuniao="R-0001"
      aoCriarAnalise={aoCriarAnalise}
    />,
  );
  return { api, aoCriarAnalise };
}

describe("Próximos passos na revisão da ata", () => {
  it("abre a análise de impacto da necessidade, ligada à demanda", async () => {
    const { api, aoCriarAnalise } = montarAta();
    const passo = await screen.findByRole("group", { name: "Próximo passo PP-1" });
    fireEvent.click(
      within(passo).getByRole("button", { name: "Analisar impacto da necessidade N-1" }),
    );
    await waitFor(() =>
      expect(api.analisarProximoPasso).toHaveBeenCalledWith(PROJETO, "R-0001", "PP-1", {
        tipo: "impacto",
        necessidade: "N-1",
      }),
    );
    await waitFor(() => expect(aoCriarAnalise).toHaveBeenCalledWith(analiseCriadaFicticia()));
  });

  it("pergunta ao sistema com o texto do passo para ajustar", async () => {
    const { api, aoCriarAnalise } = montarAta();
    fireEvent.click(await screen.findByRole("button", { name: "Perguntar ao sistema…" }));
    const campo = screen.getByLabelText("Pergunta ao sistema (PP-1)");
    expect(campo).toHaveValue("Analisar o impacto da trava nos outros canais");
    fireEvent.change(campo, { target: { value: "Quais canais cancelam fatura?" } });
    fireEvent.click(screen.getByRole("button", { name: "Perguntar" }));
    await waitFor(() =>
      expect(api.analisarProximoPasso).toHaveBeenCalledWith(PROJETO, "R-0001", "PP-1", {
        tipo: "perguntar",
        pergunta: "Quais canais cancelam fatura?",
      }),
    );
    await waitFor(() => expect(aoCriarAnalise).toHaveBeenCalled());
  });

  it("mostra a recusa do app", async () => {
    const { api, aoCriarAnalise } = montarAta();
    api.analisarProximoPasso.mockRejectedValueOnce(
      "a demanda D-0001 está encerrada; reabra-a para ligar uma análise nova",
    );
    fireEvent.click(
      await screen.findByRole("button", { name: "Analisar impacto da necessidade N-1" }),
    );
    expect(await screen.findByRole("alert")).toHaveTextContent("está encerrada");
    expect(aoCriarAnalise).not.toHaveBeenCalled();
  });

  it("no rascunho, os próximos passos ainda não abrem análises", async () => {
    montarAta(ataFicticia({ itens: [NECESSIDADE, PROXIMO_PASSO] }));
    expect(await screen.findByTestId("item-PP-1")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /Analisar impacto/ })).toBeNull();
  });
});

describe("Próximos passos no painel da demanda", () => {
  const passo = {
    id: "PP-1",
    reuniao: "R-0002",
    data: "2026-11-05",
    texto: "Levantar o impacto do parcelamento",
    responsavel: null,
    prazo: null,
    referencias: ["N-3", "D-1"],
  };

  it("viram botões que abrem a análise ligada à demanda", async () => {
    const falsa = apiConhecimentoFalsa();
    falsa.api.painelDemanda.mockResolvedValue(painelFicticio({ proximosPassos: [passo] }));
    const reuniao = apiReuniaoFalsa();
    const navegar = vi.fn();
    render(
      <PainelDemanda
        api={falsa.api}
        projeto={PROJETO}
        demanda="D-0001"
        navegar={navegar}
        apiReuniao={reuniao}
      />,
    );
    const regiao = await screen.findByRole("region", { name: "Próximos passos" });
    expect(regiao).toHaveTextContent("Levantar o impacto do parcelamento");
    fireEvent.click(
      within(regiao).getByRole("button", { name: "Analisar impacto da necessidade N-3" }),
    );
    await waitFor(() =>
      expect(reuniao.analisarProximoPasso).toHaveBeenCalledWith(PROJETO, "R-0002", "PP-1", {
        tipo: "impacto",
        necessidade: "N-3",
      }),
    );
    await waitFor(() =>
      expect(navegar).toHaveBeenCalledWith({
        tipo: "analise",
        projeto: PROJETO,
        aberta: {
          id: "A-0009",
          abertura: { tipo: "nova", promptInicial: "Inicie a análise A-0009 pelo ctower" },
        },
      }),
    );
  });

  it("com a demanda encerrada, não há botões", async () => {
    const falsa = apiConhecimentoFalsa();
    falsa.api.painelDemanda.mockResolvedValue(
      painelFicticio({
        demanda: { ...demandasFicticias()[0], status: "encerrada" },
        proximosPassos: [passo],
      }),
    );
    render(
      <PainelDemanda
        api={falsa.api}
        projeto={PROJETO}
        demanda="D-0001"
        navegar={vi.fn()}
        apiReuniao={apiReuniaoFalsa()}
      />,
    );
    const regiao = await screen.findByRole("region", { name: "Próximos passos" });
    expect(regiao).toHaveTextContent("PP-1");
    expect(within(regiao).queryByRole("button")).toBeNull();
  });
});

describe("necessidadesDe", () => {
  it("separa as necessidades das outras referências", () => {
    expect(necessidadesDe({ id: "PP-1", texto: "x", referencias: ["N-3", "D-1", "N-10"] })).toEqual(
      ["N-3", "N-10"],
    );
  });
});
