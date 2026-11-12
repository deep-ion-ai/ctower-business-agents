import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import "@xterm/xterm/css/xterm.css";

/**
 * O que o componente do terminal usa da tela (xterm.js). Separado para os testes
 * trocarem por uma tela falsa: o xterm.js precisa de canvas e medidas que o jsdom não tem.
 */
export interface TelaTerminal {
  abrirEm(elemento: HTMLElement): void;
  escrever(texto: string): void;
  /** Teclas e colagens do analista (inclusive as respostas do terminal, como a posição do cursor). */
  aoDigitar(f: (dados: string) => void): void;
  /** Ajusta linhas e colunas ao tamanho do elemento e devolve o novo tamanho. */
  ajustar(): { linhas: number; colunas: number };
  limpar(): void;
  focar(): void;
  descartar(): void;
}

/** Lê um token de theme.css; o terminal é sempre escuro, nos dois temas (#85). */
function token(nome: string): string | undefined {
  return getComputedStyle(document.documentElement).getPropertyValue(nome).trim() || undefined;
}

export function criarTelaXterm(): TelaTerminal {
  const term = new Terminal({
    cursorBlink: true,
    convertEol: false,
    fontFamily: token("--mono"),
    fontSize: 13,
    theme: {
      background: token("--term-bg"),
      foreground: token("--term-fg"),
      cursor: token("--term-text"),
      selectionBackground: token("--term-selection"),
    },
    scrollback: 5000,
  });
  const fit = new FitAddon();
  term.loadAddon(fit);
  return {
    abrirEm: (el) => term.open(el),
    escrever: (texto) => term.write(texto),
    aoDigitar: (f) => {
      term.onData(f);
    },
    ajustar: () => {
      try {
        fit.fit();
      } catch {
        // Elemento ainda sem tamanho (escondido): fica o tamanho atual.
      }
      return { linhas: term.rows, colunas: term.cols };
    },
    limpar: () => term.reset(),
    focar: () => term.focus(),
    descartar: () => term.dispose(),
  };
}
