import { describe, expect, it } from "vitest";
import type { AlteracaoPreparo } from "./analise";
import { descreverItem, outrasInstalacoes, porArquivo } from "./PrepararAmbiente";

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
