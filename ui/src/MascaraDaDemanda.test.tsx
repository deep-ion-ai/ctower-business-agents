import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { MascaraDaDemanda } from "./MascaraDaDemanda";
import { apiConfiguracoesFalsa, configuracoesFicticias } from "./test/configuracoesFalso";

function montar(mascararDemanda?: boolean, api = apiConfiguracoesFalsa()) {
  const projeto = { ...configuracoesFicticias().projetos[0], mascararDemanda };
  const aoMudar = vi.fn();
  render(<MascaraDaDemanda api={api} projeto={projeto} aoMudar={aoMudar} />);
  const caixa = screen.getByRole("checkbox", { name: /Mascarar CPF, CNPJ, e-mail e telefone/ });
  return { api, aoMudar, caixa };
}

describe("MascaraDaDemanda", () => {
  it("vem ligada por padrão (cadastro sem o campo)", () => {
    const { caixa } = montar(undefined);
    expect(caixa).toBeChecked();
    expect(screen.getByText(/O texto original continua na base/)).toBeInTheDocument();
  });

  it("desliga e grava a preferência do projeto", async () => {
    const { api, aoMudar, caixa } = montar(true);
    fireEvent.click(caixa);
    await waitFor(() => expect(aoMudar).toHaveBeenCalled());
    expect(api.salvarMascararDemanda).toHaveBeenCalledWith("faturamento-exemplo", false);
    expect(caixa).not.toBeChecked();
  });

  it("mostra a falha e mantém o valor gravado", async () => {
    const api = apiConfiguracoesFalsa();
    api.salvarMascararDemanda.mockRejectedValueOnce("o projeto não está cadastrado");
    const { caixa } = montar(false, api);
    expect(caixa).not.toBeChecked();
    fireEvent.click(caixa);
    expect(await screen.findByText("o projeto não está cadastrado")).toBeInTheDocument();
    expect(caixa).not.toBeChecked();
  });
});
