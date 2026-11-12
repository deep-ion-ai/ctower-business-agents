import { act } from "react";
import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { AvisoAlteracao } from "./AvisoAlteracao";
import { AmbienteDoSistema } from "./PrepararAmbiente";
import type {
  AmbienteClaude,
  ArquivoAlterado,
  AvisoAlteracao as Aviso,
  AvisoAnalise,
  PlanoPreparo,
  SituacaoProtecao,
} from "./analise";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(), Channel: vi.fn() }));

const PKG: ArquivoAlterado = {
  sessao: "s1",
  pasta: "/codigo/faturamento",
  caminho: "db/03_pkg_cobranca.sql",
  tipo: "modificado",
  hashAtual: "abc",
  desfazivel: true,
  diff: "--- a/db/03_pkg_cobranca.sql\n+++ b/db/03_pkg_cobranca.sql\n@@ -1 +1 @@\n-v_multa := 0.02;\n+v_multa := 0.05;\n",
  binario: false,
};

const NOVO: ArquivoAlterado = {
  ...PKG,
  caminho: "db/04_novo.sql",
  tipo: "criado",
  hashAtual: "def",
  diff: "--- /dev/null\n+++ b/db/04_novo.sql\n@@ -0,0 +1 @@\n+select 1;\n",
};

function montar(inicial: Aviso) {
  const avisos: ((a: AvisoAnalise) => void)[] = [];
  const parar = vi.fn();
  const api = {
    alteracoes: vi.fn(() => Promise.resolve(inicial)),
    desfazer: vi.fn(() => Promise.resolve(["db/03_pkg_cobranca.sql"])),
    manter: vi.fn(() => Promise.resolve(["db/03_pkg_cobranca.sql"])),
  };
  const vigiar = vi.fn((_s: string, f: (a: AvisoAnalise) => void) => {
    avisos.push(f);
    return Promise.resolve(parar);
  });
  const r = render(
    <AvisoAlteracao api={api} vigiar={vigiar} sistema="faturamento-exemplo" analise="A-0001" />,
  );
  const avisar = (a: AvisoAnalise) => avisos.forEach((f) => f(a));
  return { api, vigiar, parar, avisar, ...r };
}

describe("AvisoAlteracao", () => {
  it("sem alteração, não mostra nada; aparece quando a vigia avisa", async () => {
    const { api, avisar, container } = montar({ arquivos: [], avisos: [] });
    await waitFor(() => expect(api.alteracoes).toHaveBeenCalledTimes(1));
    expect(container).toBeEmptyDOMElement();

    // Aviso de outra análise ou de outro tipo de mudança não relê.
    act(() => avisar({ tipo: "mudanca", analise: "A-0002", mudanca: "alteracao", versao: 0 }));
    act(() => avisar({ tipo: "mudanca", analise: "A-0001", mudanca: "afirmacao", versao: 3 }));
    expect(api.alteracoes).toHaveBeenCalledTimes(1);

    api.alteracoes.mockResolvedValue({ arquivos: [PKG], avisos: [] });
    act(() => avisar({ tipo: "mudanca", analise: "A-0001", mudanca: "alteracao", versao: 0 }));
    const aviso = await screen.findByRole("alert", { name: "Aviso de alteração" });
    expect(within(aviso).getByText("db/03_pkg_cobranca.sql")).toBeInTheDocument();
    expect(within(aviso).getByText("modificado")).toHaveAttribute("data-tone", "attention");
  });

  it("mostra o diff e desfaz só depois da confirmação", async () => {
    const { api } = montar({ arquivos: [PKG, NOVO], avisos: ["pasta fora do git"] });
    expect(await screen.findByText("A pasta do sistema foi alterada nesta análise")).toBeVisible();
    expect(screen.getByText("pasta fora do git")).toBeInTheDocument();

    fireEvent.click(screen.getAllByRole("button", { name: "Ver o diff" })[0]);
    const diff = screen.getByLabelText("Diff de db/03_pkg_cobranca.sql");
    expect(diff.textContent).toContain("+v_multa := 0.05;");
    fireEvent.click(screen.getByRole("button", { name: "Esconder o diff" }));
    expect(screen.queryByLabelText("Diff de db/03_pkg_cobranca.sql")).toBeNull();

    // Desfazer um: pede confirmação; cancelar não faz nada.
    fireEvent.click(screen.getAllByRole("button", { name: "Desfazer" })[0]);
    const dialogo = screen.getByRole("dialog", { name: "Confirmar" });
    expect(dialogo.textContent).toContain("Desfazer db/03_pkg_cobranca.sql?");
    fireEvent.click(within(dialogo).getByRole("button", { name: "Cancelar" }));
    expect(api.desfazer).not.toHaveBeenCalled();

    // Desfazer todas.
    api.alteracoes.mockResolvedValue({ arquivos: [], avisos: [] });
    fireEvent.click(screen.getByRole("button", { name: "Desfazer todas" }));
    expect(screen.getByRole("dialog").textContent).toContain("Desfazer 2 arquivos?");
    fireEvent.click(screen.getByRole("button", { name: "Sim, desfazer" }));
    await waitFor(() =>
      expect(api.desfazer).toHaveBeenCalledWith("faturamento-exemplo", "A-0001", [
        {
          sessao: "s1",
          pasta: "/codigo/faturamento",
          caminho: "db/03_pkg_cobranca.sql",
          hashAtual: "abc",
        },
        { sessao: "s1", pasta: "/codigo/faturamento", caminho: "db/04_novo.sql", hashAtual: "def" },
      ]),
    );
    // Relê depois de desfazer: sem alterações, o aviso some.
    await waitFor(() => expect(screen.queryByRole("alert")).toBeNull());
  });

  it("manter também pede confirmação, e a falha aparece", async () => {
    const { api } = montar({ arquivos: [PKG], avisos: [] });
    await screen.findByText("db/03_pkg_cobranca.sql");
    api.manter.mockRejectedValueOnce("db/03_pkg_cobranca.sql mudou desde que o aviso foi mostrado");
    fireEvent.click(screen.getByRole("button", { name: "Manter" }));
    fireEvent.click(screen.getByRole("button", { name: "Sim, manter" }));
    expect(await screen.findByText(/mudou desde que o aviso foi mostrado/)).toBeInTheDocument();
    expect(api.manter).toHaveBeenCalledTimes(1);
    expect(api.desfazer).not.toHaveBeenCalled();
  });

  it("arquivo sem conteúdo anterior não oferece desfazer", async () => {
    montar({
      arquivos: [{ ...PKG, desfazivel: false, diff: null, hashAtual: null }],
      avisos: [],
    });
    await screen.findByText("db/03_pkg_cobranca.sql");
    expect(screen.queryByRole("button", { name: "Desfazer" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Ver o diff" })).toBeNull();
    expect(screen.getByRole("button", { name: "Manter" })).toBeDisabled();
    expect(screen.getByText(/confira pelo git/)).toBeInTheDocument();
  });

  it("para a vigia ao desmontar", async () => {
    const { parar, unmount, vigiar } = montar({ arquivos: [], avisos: [] });
    await waitFor(() => expect(vigiar).toHaveBeenCalled());
    await act(async () => undefined);
    unmount();
    expect(parar).toHaveBeenCalled();
  });
});

describe("Proteção opcional no preparo do ambiente", () => {
  function apiAmbiente() {
    const situacao: SituacaoProtecao = {
      sistema: "faturamento-exemplo",
      ligada: true,
      completa: true,
      arquivo: "/home/ana/.claude/settings.json",
      regras: [],
      negadasPeloHook: ["Bash", "PowerShell"],
    };
    let pronto = false;
    return {
      verificarAmbiente: vi.fn((): Promise<AmbienteClaude> =>
        Promise.resolve(
          pronto ? { pronto: true, pendencias: [] } : { pronto: false, pendencias: ["hooks"] },
        ),
      ),
      planejarPreparo: vi.fn((): Promise<PlanoPreparo> =>
        Promise.resolve({ diretorio: "/c", alteracoes: [], gravado: false }),
      ),
      prepararAmbiente: vi.fn((): Promise<PlanoPreparo> => {
        pronto = true;
        return Promise.resolve({ diretorio: "/c", alteracoes: [], gravado: true });
      }),
      mudarProtecao: vi.fn(() => Promise.resolve(situacao)),
    };
  }

  it("vem desligada e só liga com a caixa marcada", async () => {
    const api = apiAmbiente();
    render(<AmbienteDoSistema api={api} sistema="faturamento-exemplo" />);
    fireEvent.click(await screen.findByRole("button", { name: "Ver o que será alterado" }));
    const caixa = await screen.findByRole("checkbox");
    expect(caixa).not.toBeChecked();
    fireEvent.click(caixa);
    expect(screen.getByText(/Proteção contra edição: regra/)).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Concordo, preparar o ambiente" }));
    await waitFor(() =>
      expect(api.mudarProtecao).toHaveBeenCalledWith("faturamento-exemplo", true),
    );
    expect(await screen.findByText(/está preparado para este sistema/)).toBeInTheDocument();
  });

  it("sem marcar, prepara sem a proteção", async () => {
    const api = apiAmbiente();
    render(<AmbienteDoSistema api={api} sistema="faturamento-exemplo" />);
    fireEvent.click(await screen.findByRole("button", { name: "Ver o que será alterado" }));
    const concordo = await screen.findByRole("button", { name: "Concordo, preparar o ambiente" });
    await waitFor(() => expect(concordo).toBeEnabled());
    fireEvent.click(concordo);
    await waitFor(() => expect(api.prepararAmbiente).toHaveBeenCalled());
    await act(async () => undefined);
    expect(await screen.findByText(/está preparado para este sistema/)).toBeInTheDocument();
    expect(api.prepararAmbiente).toHaveBeenCalled();
    expect(api.mudarProtecao).not.toHaveBeenCalled();
  });
});
