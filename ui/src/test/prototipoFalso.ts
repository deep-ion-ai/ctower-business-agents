import type { AnaliseCriada } from "../analise";
import type { ApiPrototipo, PapelToken, TokensVista } from "../prototipo";

/** Tokens fictícios do "Faturamento Exemplo": um papel do código, um a confirmar. */
export function tokensFicticios(estado: TokensVista["estado"] = "rascunho"): TokensVista {
  const papeis: PapelToken[] = [
    {
      nome: "cor-primaria",
      categoria: "cor",
      descricao: "Cor principal das ações",
      situacao: "encontrado",
      valor: "var(--fat-azul)",
      token: "--fat-azul",
      origem: { arquivo: "web/estilos/tema.css", linha: 3 },
    },
    {
      nome: "cor-erro",
      categoria: "cor",
      descricao: "Mensagens de erro",
      situacao: "a-confirmar",
      nota: "o código calcula a cor com darken()",
    },
    {
      nome: "raio",
      categoria: "raio",
      descricao: "Cantos de campos e botões",
      situacao: "informado",
      valor: "4px",
    },
  ];
  return {
    estado,
    tokens: [
      {
        nome: "--fat-azul",
        original: "--fat-azul",
        categoria: "cor",
        valor: "#1f4e8c",
        origem: { arquivo: "web/estilos/tema.css", linha: 3 },
      },
    ],
    papeis,
    css: ":root {\n  --prototipo-cor-primaria: var(--fat-azul);\n}\n",
    aConfirmar: estado === "aprovado" ? 0 : 1,
    extraidoMs: 1,
    atualizadoMs: 2,
    aprovadoMs: estado === "aprovado" ? 3 : null,
    arquivo: "projetos/faturamento-exemplo/prototipo/tokens.css",
  };
}

/** API do protótipo fictícia, sem tokens extraídos até "Extrair do código". */
export function apiPrototipoFalsa(inicial: TokensVista | null = null) {
  const api = {
    criarAnaliseDePrototipo: vi.fn(
      (): Promise<AnaliseCriada> =>
        Promise.resolve({
          analise: {
            id: "A-0009",
            titulo: "Protótipo · Parcelamento para pessoa física",
            fluxo: "prototipo" as const,
            estado: "criada" as const,
            atualizadaMs: 9,
          },
          promptInicial: "Inicie a análise A-0009 pelo ctower",
        }),
    ),
    tokens: vi.fn((): Promise<TokensVista | null> => Promise.resolve(inicial)),
    extrairTokens: vi.fn((): Promise<TokensVista> => Promise.resolve(tokensFicticios())),
    informarToken: vi.fn(
      (_projeto: string, papel: string, valor?: string): Promise<TokensVista> => {
        const t = tokensFicticios();
        const p = t.papeis.find((x) => x.nome === papel);
        if (p && valor) Object.assign(p, { situacao: "informado", valor, nota: undefined });
        return Promise.resolve({ ...t, aConfirmar: t.papeis.filter((x) => !x.valor).length });
      },
    ),
    aprovarTokens: vi.fn((): Promise<TokensVista> => Promise.resolve(tokensFicticios("aprovado"))),
  } satisfies ApiPrototipo;
  return { api };
}
