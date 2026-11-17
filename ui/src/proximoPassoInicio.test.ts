import { naoAprovada, proximoPasso } from "./proximoPassoInicio";
import { inicioFicticio } from "./test/conhecimentoFalso";

describe("proximoPasso", () => {
  const base = inicioFicticio();
  const [impacto, multa] = base.recentes;
  const projeto = base.projetos[0];

  it("não sugere nada sem base ou sem projeto (é o caso do primeiro uso)", () => {
    expect(proximoPasso(inicioFicticio({ base: null }), {})).toBeNull();
    expect(proximoPasso(inicioFicticio({ projetos: [] }), {})).toBeNull();
  });

  it("o agente que espera vem antes da revisão, e a revisão antes das regras a revalidar", () => {
    const concluida = { ...multa, id: "A-0003", estado: "concluida" as const };
    expect(
      proximoPasso(inicioFicticio({ emAndamento: [impacto, concluida], recentes: [] }), {}),
    ).toMatchObject({ tipo: "responder" });
    const semSessao = { ...impacto, sessao: null };
    expect(
      proximoPasso(inicioFicticio({ emAndamento: [semSessao, concluida], recentes: [] }), {}),
    ).toMatchObject({ tipo: "revisar", analise: { id: "A-0003" } });
    expect(
      proximoPasso(inicioFicticio({ emAndamento: [semSessao], recentes: [] }), {}),
    ).toMatchObject({ tipo: "revalidar", regras: 1 });
  });

  it("depois das regras, o preparo do ambiente e a retomada da análise parada", () => {
    const semRevalidar = { ...projeto, aRevalidar: 0 };
    const parada = { ...impacto, sessao: null };
    const estado = inicioFicticio({ projetos: [semRevalidar], emAndamento: [parada], recentes: [] });
    expect(proximoPasso(estado, { [projeto.id]: { pronto: false, pendencias: [] } })).toMatchObject({
      tipo: "preparar",
    });
    expect(proximoPasso(estado, { [projeto.id]: { pronto: true, pendencias: [] } })).toMatchObject({
      tipo: "retomar",
    });
    // Ambiente ainda não conferido: não sugere o preparo.
    expect(proximoPasso(estado, {})).toMatchObject({ tipo: "retomar" });
  });

  it("sem pendência, não sugere nada; sem análise nenhuma, a primeira", () => {
    const semRevalidar = { ...projeto, aRevalidar: 0 };
    expect(
      proximoPasso(inicioFicticio({ projetos: [semRevalidar], emAndamento: [], recentes: [multa] }), {}),
    ).toBeNull();
    expect(
      proximoPasso(
        inicioFicticio({
          projetos: [{ ...semRevalidar, analises: 0 }],
          emAndamento: [],
          recentes: [],
        }),
        {},
      ),
    ).toMatchObject({ tipo: "primeira-analise" });
    expect(naoAprovada(multa)).toBe(false);
  });
});
