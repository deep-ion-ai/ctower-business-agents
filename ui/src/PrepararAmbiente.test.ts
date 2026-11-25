import { describe, expect, it } from "vitest";
import type { AlteracaoPreparo } from "./analise";
import { descreverAcao, descreverItem, outrasInstalacoes, porArquivo } from "./PrepararAmbiente";

const local = "/codigo/fat/.claude/settings.local.json";
const estado = "/home/ana/.claude.json";

describe("plano do preparo", () => {
  it("agrupa as alterações por arquivo, na ordem do plano (#304)", () => {
    const alteracoes: AlteracaoPreparo[] = [
      {
        arquivo: estado,
        item: { tipo: "servidorMcp", chave: "/codigo/fat" },
        acao: "adicionado",
      },
      {
        arquivo: local,
        item: { tipo: "hook", evento: "Stop" },
        acao: "adicionado",
      },
      {
        arquivo: estado,
        item: { tipo: "confianca", chave: "/codigo/fat" },
        acao: "adicionado",
      },
      {
        arquivo: "/codigo/fat/.git/info/exclude",
        item: { tipo: "exclusaoGit", raiz: "/codigo/fat" },
        acao: "adicionado",
      },
    ];
    const grupos = porArquivo(alteracoes);
    expect(grupos.map(([a, itens]) => [a, itens.length])).toEqual([
      [estado, 2],
      [local, 1],
      ["/codigo/fat/.git/info/exclude", 1],
    ]);
    expect(descreverItem(alteracoes[3].item)).toContain("fora do git");
  });

  it("mostra os workflows, as permissões e a linha deles no info/exclude (#370)", () => {
    expect(
      descreverItem({ tipo: "workflow", nome: "cba-impacto", raiz: "/codigo/wt" }),
    ).toContain("Workflow cba-impacto das análises em /codigo/wt");
    expect(descreverItem({ tipo: "permissaoWorkflow", regra: "Workflow(cba-impacto)" })).toContain(
      "Workflow(cba-impacto)",
    );
    expect(descreverItem({ tipo: "exclusaoWorkflows", raiz: "/codigo/fat" })).toContain(
      ".claude/workflows/cba-*.js",
    );
    expect(descreverAcao("preservado")).toContain("sem ser sobrescrito");
    expect(descreverAcao("atualizado")).toBe("atualizado");
    expect(descreverAcao("adicionado")).toBe("adicionado");
  });
});

describe("outras instalações", () => {
  it("conta uma vez o mesmo executável com separadores diferentes", () => {
    const alteracoes: AlteracaoPreparo[] = [
      {
        arquivo: "C:/u/.claude.json",
        item: { tipo: "servidorMcp", chave: "C:/p" },
        acao: "atualizado",
        outraInstalacao: "C:\\dev\\cba.exe",
      },
      {
        arquivo: "C:/p/.claude/settings.local.json",
        item: { tipo: "hook", evento: "Stop" },
        acao: "atualizado",
        outraInstalacao: "C:/dev/cba.exe",
      },
    ];
    expect(outrasInstalacoes(alteracoes)).toEqual(["C:\\dev\\cba.exe"]);
  });
});
