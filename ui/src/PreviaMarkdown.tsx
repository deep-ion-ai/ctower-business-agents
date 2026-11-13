import Markdown from "markdown-to-jsx";
import {
  Children,
  createContext,
  createElement,
  memo,
  useContext,
  type ComponentType,
  type ReactNode,
} from "react";
import { OPCOES_MARKDOWN_BASE } from "./markdownOpcoes";

/** Nomes que o texto aceita; vazio quando o texto não tem variáveis (não marca nenhum). */
const NomesAceitos = createContext<ReadonlySet<string>>(new Set());
/** Nomes dos marcadores do texto em renderização, na ordem das fichas. */
const NomesDaPrevia = createContext<string[]>([]);

const MARCADOR = /\{\{([a-z0-9_]+)\}\}/g;
// Os marcadores viram fichas com caracteres de uso privado, que não significam nada
// em Markdown: um `_` de `{{a_b}}` não pode virar ênfase.
const ABRE = "";
const FECHA = "";
const FICHA = /(\d+)/g;

/** Comentário HTML no começo do texto: a instrução para quem edita. */
const COMENTARIO_INICIAL = /^\s*<!--([\s\S]*?)-->[ \t]*\r?\n?/;

export interface TextoSeparado {
  /** Conteúdo do comentário inicial, sem as marcas, ou `null`. */
  nota: string | null;
  corpo: string;
}

export function separarNota(texto: string): TextoSeparado {
  const m = COMENTARIO_INICIAL.exec(texto);
  if (!m) return { nota: null, corpo: texto };
  return { nota: m[1].trim(), corpo: texto.slice(m[0].length) };
}

function Chip({ nome }: { nome: string }) {
  const aceitos = useContext(NomesAceitos);
  const desconhecido = aceitos.size > 0 && !aceitos.has(nome);
  return (
    <span
      className="chip-variavel"
      data-desconhecido={desconhecido ? "true" : undefined}
      title={desconhecido ? "Este texto não aceita este marcador" : undefined}
    >
      {`{{${nome}}}`}
    </span>
  );
}

/** Troca as fichas pelos chips. */
function comChips(filhos: ReactNode, nomes: string[]): ReactNode {
  return Children.map(filhos, (f) => {
    if (typeof f !== "string" || !f.includes(ABRE)) return f;
    // `split` com grupo alterna texto e número da ficha.
    return f
      .split(FICHA)
      .map((p, i) => (i % 2 === 0 ? p : <Chip key={i} nome={nomes[Number(p)] ?? "?"} />));
  });
}

/** Elemento que troca os marcadores do seu texto direto por chips. */
function comFichas(tag: string): ComponentType<{ children?: ReactNode }> {
  return function ElementoComChips({ children, ...props }) {
    const nomes = useContext(NomesDaPrevia);
    return createElement(tag, props, comChips(children, nomes));
  };
}

/** Na prévia nada navega: o link aparece como texto. */
function SemLink({ children, href }: { children?: ReactNode; href?: string }) {
  return (
    <span className="link-externo" title={href}>
      {children}
    </span>
  );
}

const ELEMENTOS = [
  "p",
  "li",
  "h1",
  "h2",
  "h3",
  "h4",
  "h5",
  "h6",
  "td",
  "th",
  "strong",
  "em",
  "del",
  "code",
];
// Fora do componente: componentes novos a cada render remontariam a prévia inteira.
const OVERRIDES = {
  ...Object.fromEntries(ELEMENTOS.map((t) => [t, { component: comFichas(t) }])),
  a: { component: SemLink },
};

/**
 * Prévia do texto editado: o mesmo renderizador do {@link DocumentoBase}
 * (`markdown-to-jsx`, sem HTML cru), com o comentário inicial como nota e os
 * marcadores `{{nome}}` como chips (destacados quando o texto não os aceita).
 */
export const PreviaMarkdown = memo(function PreviaMarkdown({
  texto,
  aceitos,
}: {
  texto: string;
  /** Nomes de variáveis e marcadores aceitos pelo texto. */
  aceitos: readonly string[];
}) {
  const { nota, corpo } = separarNota(texto);
  const nomes: string[] = [];
  const ficha = (t: string) =>
    t.replace(MARCADOR, (_, nome: string) => {
      nomes.push(nome);
      return `${ABRE}${nomes.length - 1}${FECHA}`;
    });
  const notaFichada = nota === null ? null : ficha(nota);
  const corpoFichado = ficha(corpo);
  return (
    <NomesAceitos.Provider value={new Set(aceitos)}>
      <NomesDaPrevia.Provider value={nomes}>
        {notaFichada !== null && (
          <aside className="nota-previa" aria-label="Instrução para quem edita">
            <strong>Instrução para quem edita</strong>
            <p>{comChips(notaFichada, nomes)}</p>
          </aside>
        )}
        <div className="markdown">
          <Markdown options={{ ...OPCOES_MARKDOWN_BASE, overrides: OVERRIDES }}>
            {corpoFichado}
          </Markdown>
        </div>
      </NomesDaPrevia.Provider>
    </NomesAceitos.Provider>
  );
});
