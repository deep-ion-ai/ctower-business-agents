import { useEffect, useRef, useState } from "react";
import { TerminalClaude, type ControleTerminal, type EstadoTerminal } from "./TerminalClaude";
import {
  listarSistemas,
  ultimaSessao,
  type PedidoClaude,
  type SistemaResumo,
  type UltimaSessao,
} from "./terminal";

/** Exemplo de prompt pronto (os definitivos chegam com o roteiro da análise). */
export const PROMPT_RESUMIR =
  "Resuma para o solicitante, em linguagem de negócio, o que foi encontrado até aqui.";

/**
 * Tela mínima do terminal embutido: escolher o sistema, abrir o Claude Code com um
 * prompt inicial opcional ou retomar a última conversa, enviar um prompt pronto e
 * continuar no Claude Desktop. O layout definitivo, ao lado do documento, é o da tela
 * da análise.
 */
export function TerminalTeste() {
  const [sistemas, setSistemas] = useState<SistemaResumo[] | null>(null);
  const [falhaLista, setFalhaLista] = useState(false);
  const [sistema, setSistema] = useState("");
  const [prompt, setPrompt] = useState("");
  const [ultima, setUltima] = useState<UltimaSessao | null>(null);
  const [aberto, setAberto] = useState<{ chave: number; pedido: PedidoClaude } | null>(null);
  const [estado, setEstado] = useState<EstadoTerminal | null>(null);
  const controle = useRef<ControleTerminal>(null);

  useEffect(() => {
    let ativo = true;
    listarSistemas()
      .then((lista) => {
        if (!ativo) return;
        setSistemas(lista);
        if (lista.length > 0) setSistema(lista[0].id);
      })
      .catch(() => ativo && setFalhaLista(true));
    return () => {
      ativo = false;
    };
  }, []);

  useEffect(() => {
    let ativo = true;
    setUltima(null);
    if (sistema)
      ultimaSessao(sistema)
        .then((u) => ativo && setUltima(u))
        .catch(() => undefined);
    return () => {
      ativo = false;
    };
  }, [sistema, aberto]);

  const abrir = (pedido: PedidoClaude) => setAberto({ chave: Date.now(), pedido });
  const vivo = estado?.fase === "aberto";

  return (
    <section className="terminal-teste" aria-label="Terminal embutido">
      <h2>Terminal embutido</h2>
      {falhaLista && <p className="suave">Não foi possível listar os sistemas cadastrados.</p>}
      {sistemas?.length === 0 && <p className="suave">Nenhum sistema cadastrado.</p>}
      {sistemas && sistemas.length > 0 && (
        <div className="terminal-controles">
          <label>
            Sistema{" "}
            <select value={sistema} onChange={(e) => setSistema(e.target.value)}>
              {sistemas.map((s) => (
                <option key={s.id} value={s.id}>
                  {s.nome}
                </option>
              ))}
            </select>
          </label>
          <label>
            Prompt inicial{" "}
            <input
              value={prompt}
              onChange={(e) => setPrompt(e.target.value)}
              placeholder="opcional"
            />
          </label>
          <button
            type="button"
            onClick={() => abrir({ promptInicial: prompt.trim() || undefined })}
          >
            Abrir no terminal
          </button>
          {ultima && (
            <button
              type="button"
              onClick={() =>
                abrir({
                  retomada: ultima.idSessao
                    ? { modo: "sessao", id: ultima.idSessao }
                    : { modo: "continuar" },
                })
              }
            >
              Retomar a sessão anterior ({new Date(ultima.abertaEm * 1000).toLocaleString()})
            </button>
          )}
        </div>
      )}
      {aberto && sistema && (
        <>
          <div className="terminal-controles">
            <button
              type="button"
              disabled={!vivo}
              onClick={() => controle.current?.enviarPrompt(PROMPT_RESUMIR)}
            >
              Resumir para o solicitante
            </button>
            <button
              type="button"
              disabled={!vivo}
              onClick={() => controle.current?.continuarNoDesktop()}
            >
              Continuar no Claude Desktop
            </button>
            <button type="button" disabled={!vivo} onClick={() => controle.current?.encerrar()}>
              Encerrar
            </button>
          </div>
          <TerminalClaude
            key={aberto.chave}
            ref={controle}
            sistema={sistema}
            pedido={aberto.pedido}
            aoMudarEstado={setEstado}
          />
        </>
      )}
    </section>
  );
}
