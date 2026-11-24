import { parecidos } from "./demandasParecidas";
import { frescor, idadeDe } from "./issuesProjeto";

const AGORA = new Date("2026-11-24T12:00:00Z").getTime();
const atras = (ms: number) => new Date(AGORA - ms).toISOString();
const MIN = 60_000;
const DIA = 86_400_000;

describe("idade e frescor", () => {
  it("diz a idade em português", () => {
    expect(idadeDe(atras(10_000), AGORA)).toBe("agora");
    expect(idadeDe(atras(5 * MIN), AGORA)).toBe("há 5 min");
    expect(idadeDe(atras(3 * 60 * MIN), AGORA)).toBe("há 3 horas");
    expect(idadeDe(atras(DIA + 60 * MIN), AGORA)).toBe("ontem");
    expect(idadeDe(atras(9 * DIA), AGORA)).toBe("há 9 dias");
    expect(idadeDe(atras(40 * DIA), AGORA)).toBe("há 1 mês");
    expect(idadeDe(atras(100 * DIA), AGORA)).toBe("há 3 meses");
    expect(idadeDe("2026-11-20", AGORA)).toMatch(/^há [3-5] dias$/);
    expect(idadeDe("não é data", AGORA)).toBe("");
  });

  it("o frescor da leitura", () => {
    expect(frescor(atras(5 * MIN), AGORA)).toBe("conferido há 5 min");
    expect(frescor(atras(1000), AGORA)).toBe("conferido agora");
    expect(frescor("???", AGORA)).toBe("");
  });
});

describe("parecidos", () => {
  const todos = [
    { id: "#412", titulo: "Portabilidade: informar o saldo devedor ao banco de destino" },
    { id: "#352", titulo: "Portabilidade de crédito (Open Finance)" },
    { id: "#391", titulo: "Quitação antecipada pagando só o saldo devedor" },
    { id: "#409", titulo: "Lembrete de vencimento por e-mail" },
  ];

  it("acha pela palavra longa ou por duas palavras, sem o próprio alvo", () => {
    const r = parecidos(todos[0], todos).map((x) => x.id);
    // #352 divide "portabilidade"; #391 divide "saldo" e "devedor"; #409 nada.
    expect(r).toEqual(expect.arrayContaining(["#352", "#391"]));
    expect(r).not.toContain("#412");
    expect(r).not.toContain("#409");
  });

  it("ignora acento e maiúscula e respeita o limite", () => {
    const alvo = { id: "x", titulo: "QUITAÇÃO ANTECIPADA" };
    expect(parecidos(alvo, todos).map((x) => x.id)).toEqual(["#391"]);
    expect(parecidos(todos[0], todos, 1)).toHaveLength(1);
  });
});
