import { useState, type FormEvent, type KeyboardEvent, type ReactNode } from "react";
import type { ClienteConversa } from "./conversa";

/** O que o agente está fazendo, em português, com o tom do indicador. */
export interface SituacaoAgente {
  texto: string;
  tom: "running" | "attention" | "done" | "muted";
}

export interface AcaoRapida {
  id: string;
  rotulo: string;
  aoClicar: () => void;
}

interface Props {
  situacao: SituacaoAgente;
  /** Onde a conversa está (`null`: nenhuma aberta). */
  onde: ClienteConversa | null;
  /** O seletor "Aqui no app ▾" (o antigo menu "Continuar a conversa" da barra). */
  seletor: ReactNode;
  /** Mostra as ações rápidas, o botão do terminal e a caixa de mensagem. */
  mostraAcoes: boolean;
  /** A sessão do terminal embutido está viva para receber entrada. */
  habilitado: boolean;
  acoesRapidas: AcaoRapida[];
  /** Os demais pedidos prontos (botão "Mais pedidos"). */
  maisPedidos?: ReactNode;
  terminalVisivel: boolean;
  aoAlternarTerminal: () => void;
  /** Envia o texto ao terminal, como se digitado; rejeita se não foi. */
  aoEscrever: (texto: string) => Promise<void>;
  /** Avisos, o terminal e o resto da coluna. */
  children: ReactNode;
}

/**
 * Aba "Conversa com o agente" (#185): o estado do agente, o seletor de onde a conversa
 * roda, as ações rápidas, o terminal (visível por padrão, com opção de esconder sem
 * encerrar a sessão) e a caixa "Escreva ao agente…".
 */
export function ConversaAgente({
  situacao,
  onde,
  seletor,
  mostraAcoes,
  habilitado,
  acoesRapidas,
  maisPedidos,
  terminalVisivel,
  aoAlternarTerminal,
  aoEscrever,
  children,
}: Props) {
  const [texto, setTexto] = useState("");
  const [enviando, setEnviando] = useState(false);

  const enviar = () => {
    // As quebras de linha seguem: o app as digita como Ctrl+J (nova linha do Claude Code).
    const linha = texto.replace(/\r\n?/g, "\n").trim();
    if (!linha || !habilitado || enviando) return;
    setEnviando(true);
    aoEscrever(linha)
      .then(() => setTexto(""))
      // Falhou: o texto fica na caixa e o motivo aparece como alerta da coluna.
      .catch(() => undefined)
      .finally(() => setEnviando(false));
  };

  const aoSubmeter = (e: FormEvent) => {
    e.preventDefault();
    enviar();
  };

  const aoTeclar = (e: KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === "Enter" && !e.shiftKey && !e.nativeEvent.isComposing) {
      e.preventDefault();
      enviar();
    }
  };

  const dica = habilitado ? undefined : "Disponível com a conversa aberta aqui, no terminal do app.";

  return (
    <>
      <div className="conversa-cabecalho">
        <span className="ponto-conversa" data-tom={situacao.tom} aria-hidden="true" />
        <span
          className="conversa-estado"
          role="status"
          aria-label="Estado do agente"
          data-onde={onde ?? "nenhum"}
        >
          {situacao.texto}
        </span>
        {seletor}
      </div>
      {mostraAcoes && (
        <div className="acoes-rapidas" role="group" aria-label="Ações rápidas">
          {acoesRapidas.map((a) => (
            <button
              key={a.id}
              className="btn btn-sm"
              type="button"
              disabled={!habilitado}
              title={dica}
              onClick={a.aoClicar}
            >
              {a.rotulo}
            </button>
          ))}
          {maisPedidos}
          <button
            className="btn btn-sm acao-terminal"
            type="button"
            aria-pressed={terminalVisivel}
            onClick={aoAlternarTerminal}
            title={
              terminalVisivel
                ? "Esconder o terminal: a conversa continua rodando"
                : "Mostrar o terminal com a saída completa"
            }
          >
            {terminalVisivel ? "Esconder o terminal" : "Mostrar o terminal"}
          </button>
        </div>
      )}
      {mostraAcoes && !terminalVisivel && (
        <p className="muted nota-terminal">
          O terminal está escondido e a conversa continua rodando. Use a caixa abaixo ou mostre o
          terminal de novo.
        </p>
      )}
      {children}
      {mostraAcoes && (
        <form className="mensagem-agente" onSubmit={aoSubmeter}>
          <textarea
            aria-label="Escreva ao agente"
            placeholder="Escreva ao agente…"
            rows={2}
            value={texto}
            disabled={!habilitado}
            title={dica}
            onChange={(e) => setTexto(e.target.value)}
            onKeyDown={aoTeclar}
          />
          <button
            className="btn btn-primary"
            type="submit"
            disabled={!habilitado || enviando || texto.trim() === ""}
          >
            Enviar
          </button>
        </form>
      )}
    </>
  );
}
