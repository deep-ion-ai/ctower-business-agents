import { describe, expect, it } from "vitest";
import type { AlteracaoPreparo } from "./analise";
import { descreverItem, porArquivo } from "./PrepararAmbiente";

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
