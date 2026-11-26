import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { TelaConfiguracoes } from "./TelaConfiguracoes";
import { apiConfiguracoesFalsa } from "./test/configuracoesFalso";
import { apiConexoesFalsa } from "./test/conexoesFalso";

const CHAVE_SECAO = "cba.configuracoes.secao";

function montar(conexoes = apiConexoesFalsa()) {
  localStorage.setItem(CHAVE_SECAO, "conexoes");
  render(<TelaConfiguracoes api={apiConfiguracoesFalsa()} apiConexoes={conexoes} />);
  return conexoes;
}

async function cartao(nome: string) {
  return within(await screen.findByRole("listitem", { name: nome }));
}

describe("Configurações › Conexões (#437)", () => {
  afterEach(() => localStorage.clear());

  it("lista cada conexão com tipo, endereço, fonte da credencial e quem a usa", async () => {
    montar();
    const gitlab = await cartao("GitLab interno");
    expect(gitlab.getByText(/GitLab ·/)).toBeInTheDocument();
    expect(gitlab.getByText("gitlab.example.com")).toBeInTheDocument();
    expect(gitlab.getByText(/Credencial: Chaveiro do sistema \(guardada\)/)).toBeInTheDocument();
    expect(
      gitlab.getByText(
        "Usada por Faturamento Exemplo (código g/fat), Cobrança Exemplo (código g/cob)",
      ),
    ).toBeInTheDocument();
    const jira = await cartao("Jira da empresa");
    expect(
      jira.getByText(/Comando que imprime a credencial: pass show jira \| head -1/),
    ).toBeInTheDocument();
    expect(jira.getByText("Nenhum projeto usa esta conexão.")).toBeInTheDocument();
    // O token nunca aparece.
    expect(document.body.textContent).not.toContain("glpat-ficticio");
  });

  it("cria uma conexão com o formulário do descritor e guarda o token no chaveiro", async () => {
    const api = montar();
    fireEvent.click(await screen.findByRole("button", { name: "Nova conexão" }));
    const form = within(screen.getByRole("group", { name: "Nova conexão" }));
    fireEvent.change(form.getByLabelText("Tipo"), { target: { value: "jira" } });
    // O campo de opções e o formato do endereço vêm do descritor do Jira.
    expect(form.getByText("URL do servidor, como https://jira.example.com")).toBeInTheDocument();
    fireEvent.change(form.getByLabelText("Arquivo de configuração do jira-cli (opcional)"), {
      target: { value: "/tmp/jira.yml" },
    });
    fireEvent.change(form.getByLabelText("Nome"), { target: { value: "Jira homologação" } });
    fireEvent.change(form.getByLabelText("Endereço"), {
      target: { value: "https://jira-hml.example.com" },
    });
    fireEvent.change(form.getByLabelText("Token ou senha"), { target: { value: "tok-jira-novo" } });
    fireEvent.click(form.getByRole("button", { name: "Criar conexão" }));
    await waitFor(() =>
      expect(api.guardarCredencialConexao).toHaveBeenCalledWith("jira-homologa-o", "tok-jira-novo"),
    );
    expect(api.salvarConexao).toHaveBeenCalledWith({
      id: "",
      nome: "Jira homologação",
      tipo: "jira",
      endereco: "https://jira-hml.example.com",
      credencial: { fonte: "chaveiro", conta: "" },
      opcoes: { config_file: "/tmp/jira.yml" },
    });
    expect(await cartao("Jira homologação")).toBeTruthy();
    expect(screen.queryByDisplayValue("tok-jira-novo")).not.toBeInTheDocument();
  });

  it("Testar mostra cada passo da prontidão com a orientação de quem falhou", async () => {
    const api = montar();
    const gitlab = await cartao("GitLab interno");
    fireEvent.click(gitlab.getByRole("button", { name: "Testar" }));
    const passos = within(await screen.findByRole("list", { name: "gitlab-interno" }));
    expect(passos.getByText(/GitLab CLI \(glab\) instalada/)).toBeInTheDocument();
    expect(passos.getByText(/glab 1.48.0 \(\/usr\/bin\/glab\)/)).toBeInTheDocument();
    expect(passos.getByText("Confira o host e se o token vale para ele.")).toBeInTheDocument();
    expect(api.testarConexao).toHaveBeenCalledWith("gitlab-interno");
    // O resultado fica na lista, como o último teste.
    expect(await gitlab.findByText("com falha")).toBeInTheDocument();
  });

  it("não remove uma conexão em uso e diz quais projetos a usam", async () => {
    const api = montar();
    const gitlab = await cartao("GitLab interno");
    fireEvent.click(gitlab.getByRole("button", { name: "Remover GitLab interno" }));
    const dialogo = within(screen.getByRole("dialog"));
    expect(
      dialogo.getByText(/usada por Faturamento Exemplo \(código g\/fat\)/),
    ).toBeInTheDocument();
    fireEvent.click(dialogo.getByRole("button", { name: "Remover" }));
    expect(
      await screen.findByText(/não pode ser removida: ela é usada por Faturamento Exemplo, Cobrança Exemplo/),
    ).toBeInTheDocument();
    expect(screen.getByRole("listitem", { name: "GitLab interno" })).toBeInTheDocument();

    // A livre sai.
    const jira = await cartao("Jira da empresa");
    fireEvent.click(jira.getByRole("button", { name: "Remover Jira da empresa" }));
    fireEvent.click(within(screen.getByRole("dialog")).getByRole("button", { name: "Remover" }));
    await waitFor(() =>
      expect(screen.queryByRole("listitem", { name: "Jira da empresa" })).not.toBeInTheDocument(),
    );
    expect(api.removerConexao).toHaveBeenCalledWith("jira-da-empresa");
  });

  it("editar não troca o tipo e mostra que há token guardado", async () => {
    montar();
    const gitlab = await cartao("GitLab interno");
    fireEvent.click(gitlab.getByRole("button", { name: "Editar" }));
    const form = within(screen.getByRole("group", { name: "Editar conexão" }));
    expect(form.getByLabelText("Tipo")).toBeDisabled();
    expect(form.getByLabelText("Token ou senha")).toHaveAttribute(
      "placeholder",
      "guardado; digite para trocar",
    );
  });
});
