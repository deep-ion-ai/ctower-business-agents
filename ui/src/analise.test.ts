import { Channel, invoke } from "@tauri-apps/api/core";
import { apiAnaliseTauri, intervaloDe, type AvisoAnalise } from "./analise";

const canais: { onmessage: (a: AvisoAnalise) => void }[] = [];

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
  Channel: vi.fn(function (this: { onmessage: unknown }) {
    this.onmessage = undefined;
    canais.push(this as never);
  }),
}));

const mockInvoke = vi.mocked(invoke);

describe("api da tela da análise", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
    canais.length = 0;
  });

  it("prompts prontos: lista vazia enquanto o comando não existe (antes da #32)", async () => {
    mockInvoke.mockRejectedValue("Command prompts_prontos not found");
    await expect(apiAnaliseTauri.promptsProntos("faturamento-exemplo", "A-0001")).resolves.toEqual(
      [],
    );
    expect(mockInvoke).toHaveBeenCalledWith("prompts_prontos", {
      sistema: "faturamento-exemplo",
      analise: "A-0001",
    });
  });

  it("prompts prontos: repassa a lista do comando", async () => {
    const lista = [{ id: "resumir", rotulo: "Resumir", texto: "Resuma." }];
    mockInvoke.mockResolvedValue(lista);
    await expect(apiAnaliseTauri.promptsProntos("faturamento-exemplo", "A-0001")).resolves.toEqual(
      lista,
    );
    mockInvoke.mockResolvedValue(null);
    await expect(apiAnaliseTauri.promptsProntos("faturamento-exemplo", "A-0001")).resolves.toEqual(
      [],
    );
  });

  it("vigia: avisos chegam pelo canal e parar cancela a assinatura", async () => {
    mockInvoke.mockResolvedValueOnce(42).mockResolvedValue(undefined);
    const recebidos: AvisoAnalise[] = [];
    const parar = await apiAnaliseTauri.vigiar("faturamento-exemplo", (a) => recebidos.push(a));
    expect(Channel).toHaveBeenCalled();
    expect(mockInvoke).toHaveBeenCalledWith("vigiar_analises", {
      sistema: "faturamento-exemplo",
      canal: canais[0],
    });
    canais[0].onmessage({ tipo: "mudanca", analise: "A-0001", mudanca: "afirmacao", versao: 2 });
    expect(recebidos).toHaveLength(1);
    parar();
    expect(mockInvoke).toHaveBeenCalledWith("parar_vigia", { id: 42 });
    canais[0].onmessage({ tipo: "reinicio" });
    expect(recebidos).toHaveLength(1);
  });

  it("intervalo da evidência", () => {
    expect(intervaloDe("71-72")).toEqual({ inicio: 71, fim: 72 });
    expect(intervaloDe("9")).toEqual({ inicio: 9, fim: 9 });
  });
});
