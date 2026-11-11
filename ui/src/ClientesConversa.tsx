import type { ClienteConversa, OpcaoCliente, OpcoesConversa } from "./conversa";

export type Modo = "retomar" | "nova";

interface Props {
  /** `null` enquanto carrega ou se o comando falhou: só o terminal embutido. */
  opcoes: OpcoesConversa | null;
  /** Há uma sessão da análise para retomar pelo identificador. */
  temSessao: boolean;
  /** Sem o cartão do terminal embutido (a tela já mostra "Voltar à conversa aqui"). */
  semAqui?: boolean;
  /** Uma ação está em curso (o `claude --desktop` pode levar alguns segundos). */
  ocupado?: boolean;
  /** O pedido da conversa nova já é conhecido (a análise carregou). */
  podeNova?: boolean;
  aoAqui: (modo: Modo) => void;
  aoDesktop: (modo: Modo) => void;
  aoLinkTerminal: () => void;
  aoCopiar: (texto: string) => void;
}

const TITULO: Record<ClienteConversa, string> = {
  terminalEmbutido: "Aqui, no terminal do app",
  claudeDesktop: "No Claude Desktop",
  terminalDoAnalista: "No meu terminal",
};

const SO_EMBUTIDO: OpcaoCliente[] = [{ cliente: "terminalEmbutido", disponivel: true }];

/**
 * Os três clientes da conversa, o preferido primeiro (spec, seções 4.2 e 4.7): o
 * terminal embutido, o Claude Desktop (link com o pedido ou `--desktop --resume`) e o
 * terminal do analista (comando para copiar e link `claude-cli://`).
 */
export function ClientesConversa({
  opcoes,
  temSessao,
  semAqui = false,
  ocupado = false,
  podeNova = true,
  aoAqui,
  aoDesktop,
  aoLinkTerminal,
  aoCopiar,
}: Props) {
  const lista = (opcoes?.opcoes ?? SO_EMBUTIDO).filter(
    (o) => !(semAqui && o.cliente === "terminalEmbutido"),
  );
  return (
    <div className="clientes-conversa" role="group" aria-label="Clientes da conversa">
      {opcoes?.problema && (
        <p className="suave" role="alert">
          {opcoes.problema}
        </p>
      )}
      {lista.map((o) => (
        <section
          key={o.cliente}
          className="cliente-conversa"
          aria-label={TITULO[o.cliente]}
          data-cliente={o.cliente}
        >
          <h3>
            {TITULO[o.cliente]}
            {opcoes?.preferido === o.cliente && <span className="selo-preferido"> preferido</span>}
          </h3>
          {!o.disponivel && <p className="suave">{o.motivo}</p>}
          {o.disponivel && o.cliente === "terminalEmbutido" && (
            <p>
              {temSessao && (
                <button type="button" onClick={() => aoAqui("retomar")}>
                  Retomar a conversa
                </button>
              )}{" "}
              <button type="button" disabled={!podeNova} onClick={() => aoAqui("nova")}>
                Nova conversa sobre esta análise
              </button>
            </p>
          )}
          {o.disponivel && o.cliente === "claudeDesktop" && (
            <>
              <p>
                {temSessao && (
                  <button type="button" disabled={ocupado} onClick={() => aoDesktop("retomar")}>
                    Retomar no Claude Desktop
                  </button>
                )}{" "}
                <button
                  type="button"
                  disabled={ocupado || !opcoes?.linkDesktop}
                  onClick={() => aoDesktop("nova")}
                >
                  Nova conversa no Claude Desktop
                </button>
              </p>
              <p className="suave">
                O Desktop pede para você confirmar a pasta e não envia o pedido sozinho.
              </p>
            </>
          )}
          {o.disponivel && o.cliente === "terminalDoAnalista" && opcoes && (
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
          <button type="button" onClick={() => aoCopiar(c.texto)}>
            {c.rotulo}
          </button>
        </p>
      ))}
      {opcoes.linkTerminal && (
        <p>
          <button type="button" onClick={aoLink}>
            Abrir pelo link claude-cli://
          </button>
        </p>
      )}
      <p className="suave">
        Retome no seu terminal só com a conversa fechada nas outras janelas: duas cópias do
        Claude na mesma conversa gravam uma por cima da outra.
      </p>
    </>
  );
}
