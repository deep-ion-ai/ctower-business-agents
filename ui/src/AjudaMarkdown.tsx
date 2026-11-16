import Markdown from "markdown-to-jsx";
import { useEffect, useRef, type KeyboardEvent, type ReactNode, type RefObject } from "react";
import { OPCOES_MARKDOWN_BASE } from "./markdownOpcoes";

/** Exemplos do Markdown básico: o que se escreve e, ao lado, como aparece. */
export const EXEMPLOS_MARKDOWN: readonly { nome: string; fonte: string }[] = [
  { nome: "Títulos", fonte: "# Título\n## Seção\n### Subseção" },
  { nome: "Ênfase", fonte: "**negrito**, *itálico* e ~~riscado~~" },
  { nome: "Lista", fonte: "- primeiro\n- segundo\n  - subitem" },
  { nome: "Lista numerada", fonte: "1. primeiro\n2. segundo" },
  { nome: "Link", fonte: "[texto do link](https://exemplo.com)" },
  { nome: "Código", fonte: "`código` no meio do texto" },
  { nome: "Bloco de código", fonte: "```\nlinha 1\nlinha 2\n```" },
  { nome: "Citação", fonte: "> Texto citado" },
  { nome: "Tabela", fonte: "| Nome | Valor |\n| --- | --- |\n| a | 1 |\n| b | 2 |" },
  { nome: "Linha horizontal", fonte: "---" },
];

function SemLink({ children }: { children?: ReactNode }) {
  return <span className="link-externo">{children}</span>;
}
// Fora do componente: um objeto novo a cada render remontaria os exemplos.
const SEM_LINK = { a: { component: SemLink } };

const FOCAVEIS =
  'button:not([disabled]), [href], input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';

/**
 * Ajuda do editor (#105): modal com o Markdown básico (você escreve / aparece assim) e
 * o que é específico do app. Acessível: `role="dialog"`, foco preso (Tab e Shift+Tab
 * circulam dentro), `Esc` fecha e o foco volta ao botão `retorno`.
 */
export function AjudaMarkdown({
  aoFechar,
  retorno,
}: {
  aoFechar: () => void;
  /** Elemento que recebe o foco ao fechar (o botão Ajuda). */
  retorno: RefObject<HTMLElement>;
}) {
  const caixa = useRef<HTMLDivElement>(null);

  useEffect(() => {
    caixa.current?.focus();
    const alvo = retorno.current;
    return () => alvo?.focus();
  }, [retorno]);

  const teclas = (e: KeyboardEvent<HTMLDivElement>) => {
    if (e.key === "Escape") {
      e.stopPropagation();
      aoFechar();
      return;
    }
    if (e.key !== "Tab" || !caixa.current) return;
    const itens = Array.from(caixa.current.querySelectorAll<HTMLElement>(FOCAVEIS));
    if (itens.length === 0) {
      e.preventDefault();
      return;
    }
    const primeiro = itens[0];
    const ultimo = itens[itens.length - 1];
    const ativo = document.activeElement;
    if (e.shiftKey && (ativo === primeiro || ativo === caixa.current)) {
      e.preventDefault();
      ultimo.focus();
    } else if (!e.shiftKey && ativo === ultimo) {
      e.preventDefault();
      primeiro.focus();
    }
  };

  return (
    <div className="dialogo-fundo">
      <div
        ref={caixa}
        className="dialogo dialogo-ajuda"
        role="dialog"
        aria-modal="true"
        aria-labelledby="ajuda-markdown-titulo"
        tabIndex={-1}
        onKeyDown={teclas}
      >
        <header className="dialogo-ajuda-topo">
          <h3 id="ajuda-markdown-titulo">Ajuda de formatação</h3>
          <button className="btn btn-sm" type="button" aria-label="Fechar a ajuda" onClick={aoFechar}>
            ×
          </button>
        </header>

        <section aria-labelledby="ajuda-basico">
          <h4 id="ajuda-basico">Markdown básico</h4>
          <p className="muted">
            A barra aplica estas marcas por você; também dá para digitá-las. A prévia mostra o resultado.
          </p>
          <table className="ajuda-tabela">
            <thead>
              <tr>
                <th scope="col">Você escreve</th>
                <th scope="col">Aparece assim</th>
              </tr>
            </thead>
            <tbody>
              {EXEMPLOS_MARKDOWN.map((x) => (
                <tr key={x.nome}>
                  <td>
                    <pre className="ajuda-fonte" aria-label={`${x.nome}: o que você escreve`}>
                      <code>{x.fonte}</code>
                    </pre>
                  </td>
                  <td>
                    <div
                      className="markdown ajuda-exemplo"
                      role="group"
                      aria-label={`${x.nome}: como aparece`}
                    >
                      <Markdown options={{ ...OPCOES_MARKDOWN_BASE, overrides: SEM_LINK }}>
                        {x.fonte}
                      </Markdown>
                    </div>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </section>

        <section aria-labelledby="ajuda-app">
          <h4 id="ajuda-app">Específico deste app</h4>
          <h5>Variáveis e marcadores</h5>
          <p>
            <code>{"{{nome}}"}</code> é trocado por um valor quando o texto é usado: os marcadores dos
            templates são preenchidos com o conteúdo do documento, e as variáveis dos roteiros e dos
            prompts, com os dados da análise. O nome usa só letras minúsculas sem acento, números e{" "}
            <code>_</code>; espaços em volta são aceitos (<code>{"{{ id }}"}</code>). Cada texto aceita
            só as suas variáveis: elas aparecem no painel lateral, e um clique insere no cursor.
          </p>
          <h5>Comentário de instrução</h5>
          <p>
            Um comentário <code>{"<!-- ... -->"}</code> no começo do texto é instrução para quem edita:
            não vai para o agente nem para o documento, e a prévia o mostra à parte. Só vale no começo
            e precisa estar fechado com <code>{"-->"}</code>.
          </p>
          <h5>Variável desconhecida ou ausente</h5>
          <ul>
            <li>
              <strong>Desconhecida</strong> (o texto não aceita aquele nome): aparece um aviso com a
              linha, e o <code>{"{{nome}}"}</code> permanece escrito como está, sem apagar nada. Na
              prévia, ele fica destacado. Escrever fora do padrão (maiúsculas, hífen) não é variável:
              fica como texto comum.
            </li>
            <li>
              <strong>Essencial ausente</strong> (roteiros e prompts que precisam de um dado e o texto
              não o usa): aviso, e o valor é acrescentado ao fim do texto, para o agente não ficar sem
              ele.
            </li>
            <li>Os avisos orientam, mas não impedem de salvar.</li>
          </ul>
        </section>

        <div className="dialogo-acoes">
          <button className="btn btn-primary" type="button" onClick={aoFechar}>
            Fechar
          </button>
        </div>
      </div>
    </div>
  );
}
