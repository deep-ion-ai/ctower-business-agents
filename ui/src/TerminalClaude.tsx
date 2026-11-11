import { forwardRef, useCallback, useEffect, useImperativeHandle, useRef, useState } from "react";
import {
  apiTauri,
  comoErroAbertura,
  PROMPT_DESKTOP,
  type ApiTerminal,
  type ErroAbertura,
  type Fim,
  type PedidoClaude,
} from "./terminal";
import { criarTelaXterm, type TelaTerminal } from "./telaXterm";

export type EstadoTerminal =
  | { fase: "abrindo" }
  | { fase: "aberto"; id: number }
  | { fase: "encerrado"; fim: Fim }
  | { fase: "erro"; erro: ErroAbertura };

/** O que a tela que usa o terminal pode fazer com a sessão viva (via `ref`). */
export interface ControleTerminal {
  /** Envia um prompt pronto, como se o analista o tivesse digitado e apertado Enter. */
  enviarPrompt(texto: string): Promise<void>;
  /** Envia `/desktop`: a sessão continua na aba de código do Claude Desktop. */
  continuarNoDesktop(): Promise<void>;
  /** Encerra o processo do `claude`. */
  encerrar(): Promise<void>;
  /** Abre uma sessão nova (encerrando a atual), com `pedido` ou o pedido das props. */
  reabrir(pedido?: PedidoClaude): void;
}

export interface PropsTerminalClaude {
  /** Identificador do sistema cadastrado: o `claude` roda na pasta dele. */
  sistema: string;
  /** Prompt inicial, retomada e argumentos do `claude` (opcionais). */
  pedido?: PedidoClaude;
  aoMudarEstado?: (estado: EstadoTerminal) => void;
  /** Para testes. */
  api?: ApiTerminal;
  /** Para testes: a tela do terminal (padrão: xterm.js). */
  criarTela?: () => TelaTerminal;
}

/**
 * Terminal embutido com o Claude Code do analista, na pasta do sistema.
 *
 * Abre ao montar, repassa o teclado ao processo, acompanha o tamanho do elemento e
 * encerra o processo ao desmontar. Se o ambiente não está preparado ou o Claude Code
 * falta, mostra a orientação em vez do terminal. Quando a sessão termina, oferece
 * continuar a conversa ou começar outra.
 */
export const TerminalClaude = forwardRef<ControleTerminal, PropsTerminalClaude>(
  function TerminalClaude(
    { sistema, pedido, aoMudarEstado, api = apiTauri, criarTela = criarTelaXterm },
    ref,
  ) {
    const elemento = useRef<HTMLDivElement>(null);
    const tela = useRef<TelaTerminal | null>(null);
    const idAtual = useRef<number | null>(null);
    const pedidoProps = useRef(pedido);
    pedidoProps.current = pedido;
    const pedidoAtual = useRef<PedidoClaude>(pedido ?? {});
    const [abertura, setAbertura] = useState(0);
    const [estado, setEstadoInterno] = useState<EstadoTerminal>({ fase: "abrindo" });
    const aoMudar = useRef(aoMudarEstado);
    aoMudar.current = aoMudarEstado;

    const setEstado = useCallback((e: EstadoTerminal) => {
      setEstadoInterno(e);
      aoMudar.current?.(e);
    }, []);

    // A tela vive enquanto o componente está montado; as sessões passam por ela.
    useEffect(() => {
      const t = criarTela();
      tela.current = t;
      if (elemento.current) t.abrirEm(elemento.current);
      t.aoDigitar((dados) => {
        const id = idAtual.current;
        if (id !== null) api.escrever(id, dados).catch(() => undefined);
      });
      const redimensionar = () => {
        const tamanho = t.ajustar();
        const id = idAtual.current;
        if (id !== null) api.redimensionar(id, tamanho).catch(() => undefined);
      };
      const observador =
        typeof ResizeObserver !== "undefined" ? new ResizeObserver(redimensionar) : null;
      if (observador && elemento.current) observador.observe(elemento.current);
      window.addEventListener("resize", redimensionar);
      return () => {
        observador?.disconnect();
        window.removeEventListener("resize", redimensionar);
        t.descartar();
        tela.current = null;
      };
    }, [api, criarTela]);

    // Outro sistema: a sessão nova usa o pedido das props.
    useEffect(() => {
      pedidoAtual.current = pedidoProps.current ?? {};
    }, [sistema]);

    // Uma sessão por abertura (e por sistema).
    useEffect(() => {
      const t = tela.current;
      if (!t) return;
      let cancelado = false;
      let terminou = false;
      let id: number | null = null;
      t.limpar();
      setEstado({ fase: "abrindo" });
      api
        .abrir(sistema, pedidoAtual.current, t.ajustar(), (evento) => {
          if (cancelado) return;
          if (evento.tipo === "saida") {
            t.escrever(evento.texto);
          } else {
            terminou = true;
            idAtual.current = null;
            setEstado({ fase: "encerrado", fim: evento.fim });
          }
        })
        .then((novo) => {
          id = novo;
          if (cancelado) {
            api.encerrar(novo).catch(() => undefined);
            return;
          }
          if (terminou) return;
          idAtual.current = novo;
          setEstado({ fase: "aberto", id: novo });
          // O tamanho pode ter mudado enquanto abria.
          api.redimensionar(novo, t.ajustar()).catch(() => undefined);
          t.focar();
        })
        .catch((e: unknown) => {
          if (!cancelado) setEstado({ fase: "erro", erro: comoErroAbertura(e) });
        });
      return () => {
        cancelado = true;
        idAtual.current = null;
        if (id !== null && !terminou) api.encerrar(id).catch(() => undefined);
      };
    }, [api, sistema, abertura, setEstado]);

    const reabrir = useCallback(
      (novo?: PedidoClaude) => {
        pedidoAtual.current = novo ?? pedidoProps.current ?? {};
        setAbertura((n) => n + 1);
      },
      [],
    );

    useImperativeHandle(
      ref,
      () => {
        const enviarPrompt = (texto: string) => {
          const id = idAtual.current;
          if (id === null) return Promise.reject(new Error("o terminal não está aberto"));
          return api.enviarPrompt(id, texto);
        };
        return {
          enviarPrompt,
          continuarNoDesktop: () => enviarPrompt(PROMPT_DESKTOP),
          encerrar: () => {
            const id = idAtual.current;
            return id === null ? Promise.resolve() : api.encerrar(id);
          },
          reabrir,
        };
      },
      [api, reabrir],
    );

    return (
      <section className="terminal-claude" aria-label="Terminal do Claude Code">
        <Situacao
          estado={estado}
          aoContinuar={() =>
            reabrir({
              ...pedidoProps.current,
              promptInicial: undefined,
              retomada: { modo: "continuar" },
            })
          }
          aoNova={() => reabrir()}
        />
        <div ref={elemento} className="terminal-tela" data-testid="terminal-tela" />
      </section>
    );
  },
);

function Situacao({
  estado,
  aoContinuar,
  aoNova,
}: {
  estado: EstadoTerminal;
  aoContinuar: () => void;
  aoNova: () => void;
}) {
  switch (estado.fase) {
    case "abrindo":
      return (
        <p className="terminal-situacao" role="status">
          Abrindo o Claude Code…
        </p>
      );
    case "aberto":
      return null;
    case "encerrado":
      return (
        <div className="terminal-situacao" role="status">
          <p>
            A sessão terminou
            {estado.fim.sinal ? ` (${estado.fim.sinal})` : ` (código ${estado.fim.codigo})`}.
          </p>
          <button type="button" onClick={aoContinuar}>
            Continuar a conversa
          </button>{" "}
          <button type="button" onClick={aoNova}>
            Nova conversa
          </button>
        </div>
      );
    case "erro":
      return (
        <div className="terminal-situacao terminal-erro" role="alert">
          <p>{estado.erro.mensagem}</p>
          <OrientacaoErro erro={estado.erro} />
          <button type="button" onClick={aoNova}>
            Tentar de novo
          </button>
        </div>
      );
  }
}

function OrientacaoErro({ erro }: { erro: ErroAbertura }) {
  if (erro.motivo === "ambienteNaoPreparado") {
    return (
      <>
        <ul>
          {erro.pendencias.map((p) => (
            <li key={p}>{p}</li>
          ))}
        </ul>
        <p>
          O app registra o servidor MCP e os hooks na configuração do Claude ao preparar o
          ambiente do sistema. Depois disso, tente de novo.
        </p>
      </>
    );
  }
  if (erro.motivo === "claudeNaoEncontrado") {
    return (
      <ol>
        {erro.orientacao.map((p) => (
          <li key={p.texto}>
            {p.texto}
            {p.comando && <code className="comando">{p.comando}</code>}
            {p.link && <span className="link">{p.link}</span>}
          </li>
        ))}
      </ol>
    );
  }
  return null;
}
