import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { ConfigWorkflows } from "./ConfigWorkflows";
import { EditorWorkflow } from "./EditorWorkflow";
import type {
  ApiWorkflows,
  Copia,
  LeituraWorkflow,
  MudancaWorkflow,
  Validacao,
  WorkflowVista,
} from "./workflows";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(), Channel: vi.fn() }));

const PADRAO = "export const meta = {\n  name: 'cba-impacto',\n  phases: [{ title: 'Preparar' }],\n}\nphase('Preparar')\n";
const FASES = [{ titulo: "Preparar", detalhe: "retoma a análise" }];
const VALIDA: Validacao = { problemas: [], fases: FASES };

const SEM_COPIA = (origem: "global" | "projeto"): Copia => ({
  origem,
  arquivo: origem === "global" ? "/dados/workflows/impacto.js" : "/base/_templates/workflows/impacto.js",
  hashPadrao: null,
  padraoMudou: false,
  situacao: "ausente",
});

function vista(parcial: Partial<WorkflowVista> = {}): WorkflowVista {
  return {
    tipo: "impacto",
    nome: "cba-impacto",
    rotulo: "Analisar impacto",
    origemEfetiva: "padrao",
    fases: FASES,
    projeto: SEM_COPIA("projeto"),
    global: SEM_COPIA("global"),
    instalacoes: [
      {
        projeto: "faturamento",
        nome: "Faturamento",
        erro: null,
        arquivos: [
          { arquivo: "/codigo/.claude/workflows/cba-impacto.js", situacao: "ok", origem: "padrao" },
        ],
      },
    ],
    ...parcial,
  };
}

function leitura(parcial: Partial<LeituraWorkflow> = {}): LeituraWorkflow {
  return {
    tipo: "impacto",
    nome: "cba-impacto",
    rotulo: "Analisar impacto",
    destino: "projeto",
    arquivo: "/base/_templates/workflows/impacto.js",
    texto: PADRAO,
    padrao: PADRAO,
    existe: true,
    padraoMudou: false,
    versao: "v1",
    validacao: VALIDA,
    ...parcial,
  };
}

function mudanca(parcial: Partial<MudancaWorkflow> = {}): MudancaWorkflow {
  return { salvo: true, leitura: leitura(), reinstalados: 1, avisos: [], ...parcial };
}

function apiFalsa(lista: WorkflowVista[] = [vista()]): ApiWorkflows {
  return {
    workflows: vi.fn().mockResolvedValue(lista),
    ler: vi.fn().mockResolvedValue(leitura()),
    validar: vi.fn().mockResolvedValue(VALIDA),
    diferenca: vi.fn().mockResolvedValue([
      { tipo: "igual", texto: "export const meta = {" },
      { tipo: "acrescentada", texto: "// minha" },
    ]),
    duplicar: vi.fn().mockResolvedValue(mudanca()),
    salvar: vi.fn().mockResolvedValue(mudanca()),
    restaurar: vi.fn().mockResolvedValue(mudanca({ leitura: leitura({ existe: false }) })),
    levarEdicao: vi.fn().mockResolvedValue(mudanca()),
  };
}

describe("ConfigWorkflows", () => {
  it("mostra a origem em uso, as cópias e o que está instalado em cada projeto", async () => {
    const aoEditar = vi.fn();
    render(<ConfigWorkflows api={apiFalsa()} temBase recarga={0} aoEditar={aoEditar} />);
    const cartao = await screen.findByRole("region", { name: "Analisar impacto" });
    const c = within(cartao);
    expect(c.getByText("Em uso: padrão do app")).toBeInTheDocument();
    expect(c.getByText(/Fases: Preparar/)).toBeInTheDocument();
    expect(c.getByText(/com o padrão do app/)).toBeInTheDocument();
    fireEvent.click(c.getByRole("button", { name: "Ver o padrão de cba-impacto" }));
    expect(aoEditar).toHaveBeenCalledWith("impacto", null);
  });

  it("duplica para todos os projetos e abre a cópia no editor", async () => {
    const api = apiFalsa();
    const aoEditar = vi.fn();
    render(<ConfigWorkflows api={api} temBase recarga={0} aoEditar={aoEditar} />);
    fireEvent.click(
      await screen.findByRole("button", {
        name: "Duplicar cba-impacto: cópia para todos os projetos",
      }),
    );
    await waitFor(() => expect(api.duplicar).toHaveBeenCalledWith("impacto", "global"));
    expect(aoEditar).toHaveBeenCalledWith("impacto", "global");
    expect(await screen.findByText(/instalado num projeto foi atualizado/)).toBeInTheDocument();
  });

  it("sem base, não duplica na base", async () => {
    render(
      <ConfigWorkflows api={apiFalsa([vista({ projeto: null })])} temBase={false} recarga={0} aoEditar={vi.fn()} />,
    );
    expect(
      await screen.findByRole("button", { name: "Duplicar cba-impacto: cópia na base" }),
    ).toBeDisabled();
  });

  it("avisa que o padrão mudou e restaura depois de confirmar", async () => {
    const api = apiFalsa([
      vista({
        origemEfetiva: "projeto",
        projeto: { ...SEM_COPIA("projeto"), situacao: "valida", padraoMudou: true, hashPadrao: "a" },
      }),
    ]);
    const aoEditar = vi.fn();
    render(<ConfigWorkflows api={api} temBase recarga={0} aoEditar={aoEditar} />);
    expect(await screen.findByText(/O padrão do app mudou desde esta cópia/)).toBeInTheDocument();
    expect(screen.getByText("Em uso: cópia na base")).toBeInTheDocument();
    fireEvent.click(
      screen.getByRole("button", { name: "Ver a diferença da cópia na base de cba-impacto com o padrão" }),
    );
    expect(aoEditar).toHaveBeenCalledWith("impacto", "projeto", "diferenca");

    fireEvent.click(
      screen.getByRole("button", { name: "Restaurar o padrão no lugar da cópia na base de cba-impacto" }),
    );
    expect(api.restaurar).not.toHaveBeenCalled();
    fireEvent.click(await screen.findByRole("button", { name: "Apagar a cópia" }));
    await waitFor(() => expect(api.restaurar).toHaveBeenCalledWith("impacto", "projeto"));
  });

  it("mostra a cópia com erros, que não é usada", async () => {
    render(
      <ConfigWorkflows
        api={apiFalsa([
          vista({
            global: { ...SEM_COPIA("global"), situacao: "invalida", erros: "linha 2: `name` deve ser 'cba-impacto'" },
          }),
        ])}
        temBase
        recarga={0}
        aoEditar={vi.fn()}
      />,
    );
    expect(await screen.findByText(/Com erros, não é usada: linha 2/)).toBeInTheDocument();
  });

  it("oferece levar para uma cópia a edição feita no arquivo instalado", async () => {
    const api = apiFalsa([
      vista({
        instalacoes: [
          {
            projeto: "faturamento",
            nome: "Faturamento",
            erro: null,
            arquivos: [
              { arquivo: "/codigo/.claude/workflows/cba-impacto.js", situacao: "divergente", origem: "padrao" },
            ],
          },
        ],
      }),
    ]);
    render(<ConfigWorkflows api={api} temBase recarga={0} aoEditar={vi.fn()} />);
    expect(await screen.findByText("editado à mão")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Levar a edição para a cópia na base" }));
    fireEvent.click(await screen.findByRole("button", { name: "Levar a edição" }));
    await waitFor(() =>
      expect(api.levarEdicao).toHaveBeenCalledWith(
        "faturamento",
        "impacto",
        "/codigo/.claude/workflows/cba-impacto.js",
        "projeto",
      ),
    );
    expect(await screen.findByText(/é a versão em uso/)).toBeInTheDocument();
  });
});

describe("EditorWorkflow", () => {
  it("o padrão abre só para leitura, com as fases", async () => {
    const api = apiFalsa();
    vi.mocked(api.ler).mockResolvedValue(leitura({ destino: null, arquivo: null }));
    render(<EditorWorkflow api={api} tipo="impacto" destino={null} aoSair={vi.fn()} aoAlterado={vi.fn()} />);
    const campo = await screen.findByRole("textbox", { name: "Script do workflow (JavaScript)" });
    expect(campo).toHaveAttribute("readonly");
    expect(screen.queryByRole("button", { name: "Salvar" })).not.toBeInTheDocument();
    expect(within(screen.getByRole("list", { name: "Fases do workflow" })).getByText("Preparar")).toBeInTheDocument();
  });

  it("recusa ao salvar um script com erro, mostrando a linha", async () => {
    const api = apiFalsa();
    const comErro: Validacao = {
      problemas: [{ linha: 5, mensagem: "a fase 'Preparo' não está em `meta.phases`", erro: true }],
      fases: FASES,
    };
    vi.mocked(api.salvar).mockResolvedValue(
      mudanca({ salvo: false, reinstalados: 0, leitura: leitura({ validacao: comErro }) }),
    );
    const aoAlterado = vi.fn();
    render(<EditorWorkflow api={api} tipo="impacto" destino="projeto" aoSair={vi.fn()} aoAlterado={aoAlterado} />);
    const campo = await screen.findByRole("textbox", { name: "Script do workflow (JavaScript)" });
    fireEvent.change(campo, { target: { value: PADRAO.replace("phase('Preparar')", "phase('Preparo')") } });
    fireEvent.click(screen.getByRole("button", { name: "Salvar" }));
    expect(await screen.findByText(/tem erros e não foi salvo/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Linha 5" })).toBeInTheDocument();
    expect(screen.getByText(/'Preparo' não está em/)).toBeInTheDocument();
    expect(aoAlterado).not.toHaveBeenCalled();
    expect(screen.getByText("Alterações não salvas")).toBeInTheDocument();
  });

  it("salva o script válido e conta a reinstalação", async () => {
    const api = apiFalsa();
    const aoAlterado = vi.fn();
    render(<EditorWorkflow api={api} tipo="impacto" destino="global" aoSair={vi.fn()} aoAlterado={aoAlterado} />);
    const campo = await screen.findByRole("textbox", { name: "Script do workflow (JavaScript)" });
    fireEvent.change(campo, { target: { value: `${PADRAO}// minha\n` } });
    fireEvent.click(screen.getByRole("button", { name: "Salvar" }));
    await waitFor(() =>
      expect(api.salvar).toHaveBeenCalledWith("impacto", "global", `${PADRAO}// minha\n`, "v1", false),
    );
    expect(await screen.findByText(/A próxima análise usa esta versão/)).toBeInTheDocument();
    expect(aoAlterado).toHaveBeenCalled();
  });

  it("avisa do padrão novo, mostra a diferença e marca como revista", async () => {
    const api = apiFalsa();
    vi.mocked(api.ler).mockResolvedValue(leitura({ padraoMudou: true }));
    render(
      <EditorWorkflow
        api={api}
        tipo="impacto"
        destino="projeto"
        painelInicial="diferenca"
        aoSair={vi.fn()}
        aoAlterado={vi.fn()}
      />,
    );
    expect(await screen.findByText(/O padrão do app mudou desde a sua cópia/)).toBeInTheDocument();
    const dif = await screen.findByLabelText("Diferença com o padrão", { selector: "pre" });
    expect(within(dif).getByText(/minha/)).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Já revi: marcar como revista" }));
    await waitFor(() => expect(api.salvar).toHaveBeenCalledWith("impacto", "projeto", PADRAO, "v1", true));
  });
});
