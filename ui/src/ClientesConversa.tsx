import type { ClienteConversa, OpcaoCliente, OpcoesConversa } from "./conversa";

export type Modo = "retomar" | "nova";

interface Props {
  /** `null` enquanto carrega ou se o comando falhou: só o terminal embutido. */
  opcoes: OpcoesConversa | null;
  /** Há uma sessão da análise para retomar pelo identificador. */
  temSessao: boolean;
  /** Onde a conversa está agora (`null`: nenhuma aberta). */
  onde: ClienteConversa | null;
  /** O terminal embutido tem o Claude rodando. */
  vivo: boolean;
  /** Uma ação está em curso (o `claude --desktop` pode levar alguns segundos). */
  ocupado?: boolean;
  /** O pedido da conversa nova já é conhecido (a análise carregou). */
  podeNova?: boolean;
  aoAqui: (modo: Modo) => void;
  /** A conversa está fora: volta para o terminal embutido. */
  aoVoltarAqui: () => void;
  aoDesktop: (modo: Modo) => void;
  /** Encerra o terminal embutido e passa a conversa ao terminal do analista. */
  aoMeuTerminal: () => void;
  aoLinkTerminal: () => void;
  aoCopiar: (texto: string) => void;
}

export const TITULO_CLIENTE: Record<ClienteConversa, string> = {
  terminalEmbutido: "Aqui, no terminal do app",
  claudeDesktop: "No Claude Desktop",
  terminalDoAnalista: "No meu terminal",
};

const SO_EMBUTIDO: OpcaoCliente[] = [{ cliente: "terminalEmbutido", disponivel: true }];

/**
 * Os três clientes da conversa, o preferido primeiro (spec, seções 4.2 e 4.7; #130):
 * o terminal embutido, o Claude Desktop (link com o pedido, `/desktop` com a sessão
 * viva ou `--desktop --resume`) e o terminal do analista (comando para copiar e link
 * `claude-cli://`). O cliente em que a conversa está agora fica marcado.
 */
export function ClientesConversa({
  opcoes,
  temSessao,
  onde,
  vivo,
  ocupado = false,
  podeNova = true,
  aoAqui,
  aoVoltarAqui,
  aoDesktop,
  aoMeuTerminal,
  aoLinkTerminal,
  aoCopiar,
}: Props) {
  const lista = opcoes?.opcoes ?? SO_EMBUTIDO;
  const aquiAgora = onde === "terminalEmbutido";
  return (
    <div className="clientes-conversa" role="group" aria-label="Clientes da conversa">
      {opcoes?.problema && (
        <p className="muted" role="alert">
          {opcoes.problema}
        </p>
      )}
      {lista.map((o) => (
        <section
          key={o.cliente}
          className="cliente-conversa"
          aria-label={TITULO_CLIENTE[o.cliente]}
          data-cliente={o.cliente}
          data-agora={onde === o.cliente || undefined}
        >
          <h3>
            {TITULO_CLIENTE[o.cliente]}
            {onde === o.cliente && (
              <span className="badge" data-tone="done">
                {" "}
                agora
              </span>
            )}
            {opcoes?.preferido === o.cliente && <span className="badge"> preferido</span>}
          </h3>
          {!o.disponivel && <p className="muted">{o.motivo}</p>}
          {o.disponivel && o.cliente === "terminalEmbutido" && (
            <p>
              {aquiAgora && <span className="muted">A conversa está aberta aqui.</span>}
              {!aquiAgora && onde !== null && (
                <button className="btn" type="button" onClick={aoVoltarAqui}>
                  Voltar à conversa aqui
                </button>
              )}
              {onde === null && temSessao && (
                <button className="btn" type="button" onClick={() => aoAqui("retomar")}>
                  Retomar a conversa
                </button>
              )}{" "}
              {onde === null && (
                <button
                  className="btn"
                  type="button"
                  disabled={!podeNova}
                  onClick={() => aoAqui("nova")}
                >
                  Nova conversa sobre esta análise
                </button>
              )}
            </p>
          )}
          {o.disponivel && o.cliente === "claudeDesktop" && (
            <>
              <p>
                {vivo && aquiAgora && (
                  <button
                    className="btn"
                    type="button"
                    disabled={ocupado}
                    onClick={() => aoDesktop("retomar")}
                  >
                    Continuar no Claude Desktop
                  </button>
                )}
                {!(vivo && aquiAgora) && temSessao && (
                  <button
                    className="btn"
                    type="button"
                    disabled={ocupado}
                    onClick={() => aoDesktop("retomar")}
                  >
                    Retomar no Claude Desktop
                  </button>
                )}{" "}
                {!(vivo && aquiAgora) && (
                  <button
                    className="btn"
                    type="button"
                    disabled={ocupado || !opcoes?.linkDesktop}
                    onClick={() => aoDesktop("nova")}
                  >
                    Nova conversa no Claude Desktop
                  </button>
                )}
              </p>
              <p className="muted">
                {vivo && aquiAgora
                  ? "O Claude Code salva a conversa e a abre no Desktop."
                  : "O Desktop pede para você confirmar a pasta e não envia o pedido sozinho."}
              </p>
            </>
          )}
          {o.disponivel && o.cliente === "terminalDoAnalista" && vivo && aquiAgora && (
            <p>
              <button className="btn" type="button" onClick={aoMeuTerminal}>
                Continuar no meu terminal
              </button>
            </p>
          )}
          {o.disponivel &&
            o.cliente === "terminalDoAnalista" &&
            !(vivo && aquiAgora) &&
            opcoes && (
              <ComandosTerminal opcoes={opcoes} aoLink={aoLinkTerminal} aoCopiar={aoCopiar} />
            )}
        </section>
      ))}
    </div>
  );
}

function ComandosTerminal({
  opcoes,
  aoLink,
  aoCopiar,
}: {
  opcoes: OpcoesConversa;
  aoLink: () => void;
  aoCopiar: (texto: string) => void;
}) {
  const comandos: { rotulo: string; texto: string }[] = [];
  if (opcoes.comandoRetomada) {
    comandos.push({ rotulo: "Copiar o comando de retomada", texto: opcoes.comandoRetomada });
  }
  if (opcoes.comandoTerminal) {
    comandos.push({ rotulo: "Copiar o comando de conversa nova", texto: opcoes.comandoTerminal });
  }
  return (
    <>
      {comandos.map((c) => (
        <p key={c.rotulo} className="comando-copiar">
          <code className="comando">{c.texto}</code>{" "}
          <button className="btn" type="button" onClick={() => aoCopiar(c.texto)}>
            {c.rotulo}
          </button>
        </p>
      ))}
      {opcoes.linkTerminal && (
        <p>
          <button className="btn" type="button" onClick={aoLink}>
            Abrir pelo link claude-cli://
          </button>
        </p>
      )}
      <p className="muted">
        Retome no seu terminal só com a conversa fechada nas outras janelas: duas cópias do
        Claude na mesma conversa gravam uma por cima da outra.
      </p>
    </>
  );
}
