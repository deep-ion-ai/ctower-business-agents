import { useEffect, useId, useRef, useState, type KeyboardEvent } from "react";
import { Selo } from "./componentes";
import { Icon } from "./shell/icons";

/** Projeto na lista do seletor, com o que espera o analista nele. */
export interface ProjetoNoSeletor {
  id: string;
  nome: string;
  /** Itens que esperam o analista (ver {@link pendenciasNoInicio}). */
  pendencias: number;
  /** A pasta do projeto na base não pôde ser lida. */
  erro: boolean;
}

const plural = (n: number) => (n === 1 ? "1 pendência" : `${n} pendências`);

/**
 * Seletor de projeto na barra de título (#319, §5.2 da revisão de UX): mostra o
 * projeto escolhido e, aberto, a lista dos projetos com as pendências de cada um,
 * para comparar sem sair do projeto atual. É um botão com uma lista (padrão
 * WAI-ARIA listbox): setas, Home e End andam pela lista; Enter ou espaço escolhem;
 * Esc fecha e devolve o foco ao botão.
 */
export function SeletorProjeto({
  projetos,
  atual,
  aoEscolher,
}: {
  projetos: ProjetoNoSeletor[];
  atual: string;
  aoEscolher: (id: string) => void;
}) {
  const [aberto, setAberto] = useState(false);
  const [foco, setFoco] = useState(0);
  const raiz = useRef<HTMLDivElement>(null);
  const botao = useRef<HTMLButtonElement>(null);
  const opcoes = useRef<(HTMLLIElement | null)[]>([]);
  const idLista = useId();

  const escolhido = projetos.find((p) => p.id === atual);

  // Clique fora fecha a lista.
  useEffect(() => {
    if (!aberto) return;
    const fora = (e: MouseEvent) => {
      if (!raiz.current?.contains(e.target as Node)) setAberto(false);
    };
    document.addEventListener("mousedown", fora);
    return () => document.removeEventListener("mousedown", fora);
  }, [aberto]);

  useEffect(() => {
    if (aberto) opcoes.current[foco]?.focus();
  }, [aberto, foco]);

  const abrir = () => {
    const i = projetos.findIndex((p) => p.id === atual);
    setFoco(i < 0 ? 0 : i);
    setAberto(true);
  };

  const fechar = () => {
    setAberto(false);
    botao.current?.focus();
  };

  const escolher = (id: string) => {
    fechar();
    if (id !== atual) aoEscolher(id);
  };

  const teclasDaLista = (e: KeyboardEvent<HTMLUListElement>) => {
    const n = projetos.length;
    const destino =
      e.key === "ArrowDown"
        ? (foco + 1) % n
        : e.key === "ArrowUp"
          ? (foco - 1 + n) % n
          : e.key === "Home"
            ? 0
            : e.key === "End"
              ? n - 1
              : -1;
    if (destino >= 0) {
      e.preventDefault();
      setFoco(destino);
      return;
    }
    if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      const p = projetos[foco];
      if (p) escolher(p.id);
    } else if (e.key === "Escape" || e.key === "Tab") {
      if (e.key === "Escape") e.preventDefault();
      fechar();
    }
  };

  const teclasDoBotao = (e: KeyboardEvent<HTMLButtonElement>) => {
    if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      abrir();
    }
  };

  if (projetos.length === 0) {
    return <span className="seletor-projeto-vazio muted">Nenhum projeto cadastrado</span>;
  }

  return (
    <div className="seletor-projeto" ref={raiz}>
      <button
        ref={botao}
        type="button"
        className="seletor-projeto-botao"
        aria-haspopup="listbox"
        aria-expanded={aberto}
        aria-controls={aberto ? idLista : undefined}
        title="Trocar de projeto"
        onClick={() => (aberto ? setAberto(false) : abrir())}
        onKeyDown={teclasDoBotao}
      >
        <span className="sr-only">Projeto: </span>
        <span className="seletor-projeto-nome">{escolhido?.nome ?? "Escolha um projeto"}</span>
        <Icon name="chevronDown" size={12} />
      </button>
      {aberto && (
        <ul
          id={idLista}
          className="seletor-projeto-lista"
          role="listbox"
          aria-label="Projetos"
          onKeyDown={teclasDaLista}
        >
          {projetos.map((p, i) => (
            <li
              key={p.id}
              ref={(el) => {
                opcoes.current[i] = el;
              }}
              role="option"
              tabIndex={i === foco ? 0 : -1}
              aria-selected={p.id === atual}
              className="seletor-projeto-opcao"
              onClick={() => escolher(p.id)}
            >
              <span className="seletor-projeto-opcao-nome">{p.nome}</span>
              {p.erro ? (
                <Selo tom="erro">com erro</Selo>
              ) : p.pendencias > 0 ? (
                <Selo tom="atencao">{plural(p.pendencias)}</Selo>
              ) : (
                <span className="muted seletor-projeto-ok">em dia</span>
              )}
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
