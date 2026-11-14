import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { NovaReuniao } from "./NovaReuniao";
import { PainelDemanda } from "./PainelDemanda";
import { TelaInicio } from "./TelaInicio";
import { PROJETO, apiConhecimentoFalsa } from "./test/conhecimentoFalso";
import { clientesProntos } from "./test/falsos";
import { apiReuniaoFalsa } from "./test/reunioesFalso";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(), Channel: vi.fn() }));
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn(), save: vi.fn() }));
vi.mock("./clientes", () => ({ estadoClientes: vi.fn(() => Promise.resolve(clientesProntos())) }));

const TRANSCRIPT = "Silva, Maria   0:03\nBom dia, pessoal.\n\nJoão Pereira   0:30\nBom dia.";

function montar(demandaFixa?: string) {
  const conhecimento = apiConhecimentoFalsa();
  const reuniao = apiReuniaoFalsa();
  const aoCriar = vi.fn();
  render(
    <NovaReuniao
      api={conhecimento.api}
      apiReuniao={reuniao}
      projetos={[{ id: PROJETO, nome: "Faturamento Exemplo" }]}
      demandaFixa={demandaFixa}
      aoCriar={aoCriar}
      aoCancelar={vi.fn()}
    />,
  );
  return { conhecimento, reuniao, aoCriar };
}

describe("NovaReuniao", () => {
  it("mostra a prévia e, sem aceitar o aviso, não cria a reunião", async () => {
    const { reuniao, aoCriar } = montar();
    const form = screen.getByRole("form", { name: "Analisar reunião" });
    // A demanda aberta vem escolhida; a encerrada não aparece.
    const demanda = await within(form).findByRole("option", { name: "D-0001: Parcelamento PJ" });
    expect((demanda as HTMLOptionElement).selected).toBe(true);
    expect(within(form).queryByRole("option", { name: /D-0002/ })).toBeNull();

    fireEvent.change(within(form).getByLabelText("Transcript"), {
      target: { value: TRANSCRIPT },
    });
    fireEvent.click(within(form).getByRole("button", { name: "Ver a prévia" }));
    await waitFor(() =>
      expect(reuniao.previaTranscript).toHaveBeenCalledWith({ tipo: "texto", texto: TRANSCRIPT }),
    );
    const previa = await screen.findByRole("region", { name: "Prévia do transcript" });
    expect(previa).toHaveTextContent("Texto copiado do Teams · 12 fala(s) · 3 falante(s)");
    expect(previa).toHaveTextContent("Silva, Maria · 5 fala(s)");
    expect(within(previa).getByRole("list", { name: "Avisos da leitura" })).toHaveTextContent(
      "2 linha(s) antes da primeira fala foram descartadas.",
    );

    const aviso = screen.getByRole("region", { name: "Aviso de privacidade" });
    expect(aviso).toHaveTextContent("provedor de IA");
    expect(aviso).toHaveTextContent("diretório de dados do app, fora da base");
    expect(aviso).toHaveTextContent("apagado ao aprovar a ata");

    const criar = within(form).getByRole("button", { name: "Criar a reunião e iniciar a análise" });
    await waitFor(() => expect(screen.getByText(/Aceite o aviso de privacidade/)).toBeVisible());
    expect(criar).toBeDisabled();
    // Nem enviando o formulário à força.
    fireEvent.submit(form);
    expect(reuniao.criarReuniao).not.toHaveBeenCalled();
    expect(aoCriar).not.toHaveBeenCalled();

    fireEvent.click(within(aviso).getByLabelText("Li o aviso e quero seguir"));
    expect(criar).toBeEnabled();
    fireEvent.click(criar);
    await waitFor(() =>
      expect(reuniao.criarReuniao).toHaveBeenCalledWith(
        PROJETO,
        { tipo: "existente", id: "D-0001" },
        { tipo: "texto", texto: TRANSCRIPT },
        true,
      ),
    );
    await waitFor(() =>
      expect(aoCriar).toHaveBeenCalledWith(
        PROJETO,
        expect.objectContaining({
          analise: expect.objectContaining({ id: "A-0005", fluxo: "reuniao" }),
          promptInicial: "Inicie a análise A-0005 pelo ctower com reuniao R-0001",
        }),
      ),
    );
  });

  it("anexa o arquivo e cria uma demanda nova com a chave externa", async () => {
    const { reuniao } = montar();
    const form = screen.getByRole("form", { name: "Analisar reunião" });
    await within(form).findByRole("option", { name: "D-0001: Parcelamento PJ" });
    fireEvent.change(within(form).getByLabelText("Demanda"), { target: { value: "__nova__" } });
    fireEvent.change(within(form).getByLabelText("Título da demanda"), {
      target: { value: "Cancelamento de fatura antiga" },
    });
    fireEvent.change(within(form).getByLabelText("Chave externa (opcional)"), {
      target: { value: "FAT-31" },
    });

    fireEvent.click(within(form).getByRole("button", { name: "Anexar .vtt ou .docx" }));
    expect(await within(form).findByText("reuniao-01-cancelamento.vtt")).toBeInTheDocument();
    const arquivo = { tipo: "arquivo", caminho: "/home/analista/reuniao-01-cancelamento.vtt" };
    await waitFor(() => expect(reuniao.previaTranscript).toHaveBeenCalledWith(arquivo));
    await screen.findByRole("region", { name: "Prévia do transcript" });

    fireEvent.click(within(form).getByLabelText("Li o aviso e quero seguir"));
    fireEvent.click(within(form).getByRole("button", { name: "Criar a reunião e iniciar a análise" }));
    await waitFor(() =>
      expect(reuniao.criarReuniao).toHaveBeenCalledWith(
        PROJETO,
        { tipo: "nova", titulo: "Cancelamento de fatura antiga", chave: "FAT-31" },
        arquivo,
        true,
      ),
    );
  });

  it("não cria sem falas no transcript e mostra a falha do app", async () => {
    const { reuniao } = montar("D-0001");
    reuniao.previaTranscript.mockResolvedValueOnce({
      formato: "desconhecido",
      falas: 0,
      falantes: [],
      semFalante: 0,
      avisos: [{ codigo: "entrada_vazia", mensagem: "O transcript está vazio." }],
      podeCriar: false,
    });
    const form = screen.getByRole("form", { name: "Analisar reunião" });
    expect(form).toHaveTextContent("A reunião fica na demanda D-0001.");
    fireEvent.change(within(form).getByLabelText("Transcript"), { target: { value: "x" } });
    fireEvent.click(within(form).getByRole("button", { name: "Ver a prévia" }));
    expect(await screen.findByText(/Nenhuma fala encontrada/)).toBeInTheDocument();
    fireEvent.click(within(form).getByLabelText("Li o aviso e quero seguir"));
    expect(
      within(form).getByRole("button", { name: "Criar a reunião e iniciar a análise" }),
    ).toBeDisabled();

    // Com falas, mas o app recusa: a falha aparece.
    reuniao.criarReuniao.mockRejectedValueOnce("a demanda D-0001 está encerrada");
    fireEvent.change(within(form).getByLabelText("Transcript"), { target: { value: TRANSCRIPT } });
    fireEvent.click(within(form).getByRole("button", { name: "Ver a prévia" }));
    await screen.findByText(/12 fala\(s\)/);
    fireEvent.click(within(form).getByRole("button", { name: "Criar a reunião e iniciar a análise" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("a demanda D-0001 está encerrada");
  });
});

describe("Nova reunião pelo painel da demanda e pelo início", () => {
  it("o painel da demanda abre a reunião ligada à demanda e a análise nova", async () => {
    const conhecimento = apiConhecimentoFalsa();
    const reuniao = apiReuniaoFalsa();
    const navegar = vi.fn();
    render(
      <PainelDemanda
        api={conhecimento.api}
        apiReuniao={reuniao}
        projeto={PROJETO}
        demanda="D-0001"
        navegar={navegar}
      />,
    );
    fireEvent.click(await screen.findByRole("button", { name: "Analisar reunião" }));
    const form = screen.getByRole("form", { name: "Analisar reunião" });
    expect(within(form).queryByLabelText("Demanda")).toBeNull();
    fireEvent.change(within(form).getByLabelText("Transcript"), { target: { value: TRANSCRIPT } });
    fireEvent.click(within(form).getByRole("button", { name: "Ver a prévia" }));
    await screen.findByRole("region", { name: "Prévia do transcript" });
    fireEvent.click(within(form).getByLabelText("Li o aviso e quero seguir"));
    fireEvent.click(within(form).getByRole("button", { name: "Criar a reunião e iniciar a análise" }));
    await waitFor(() =>
      expect(navegar).toHaveBeenCalledWith({
        tipo: "analise",
        projeto: PROJETO,
        aberta: {
          id: "A-0005",
          abertura: {
            tipo: "nova",
            promptInicial: "Inicie a análise A-0005 pelo ctower com reuniao R-0001",
          },
        },
      }),
    );
    expect(reuniao.criarReuniao).toHaveBeenCalledWith(
      PROJETO,
      { tipo: "existente", id: "D-0001" },
      { tipo: "texto", texto: TRANSCRIPT },
      true,
    );
  });

  it("o início tem o botão de analisar reunião", async () => {
    const conhecimento = apiConhecimentoFalsa();
    render(
      <TelaInicio api={conhecimento.api} apiReuniao={apiReuniaoFalsa()} navegar={vi.fn()} />,
    );
    const botao = await screen.findByRole("button", { name: "Analisar reunião" });
    await waitFor(() => expect(botao).toBeEnabled());
    fireEvent.click(botao);
    expect(screen.getByRole("form", { name: "Analisar reunião" })).toBeInTheDocument();
    expect(screen.getByRole("region", { name: "Aviso de privacidade" })).toBeInTheDocument();
  });
});
