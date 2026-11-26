import type { AriaRole, ReactNode } from "react";
import { IconeAtencao, IconeErro } from "../icones";
import type { Acao } from "./Acoes";
import "./componentes.css";

/**
 * Estado vazio: uma frase e o próximo passo, numa linha tracejada e compacta. Seção
 * vazia não ocupa espaço de seção cheia, e tabela vazia não mostra "0 de 0": use
 * este componente no lugar da seção ou da tabela.
 */
export function EstadoVazio({
  children,
  acao,
}: {
  /** A frase ("Nenhum registro deste tipo."). */
  children: ReactNode;
  /** O próximo passo, em botão de texto ("Criar o primeiro"). */
  acao?: Acao;
}) {
  return (
    <div className="estado-vazio">
      {children}
      {acao && (
        <>
          {" "}
          <button
            type="button"
            className="link"
            disabled={acao.desabilitado}
            title={acao.dica}
            onClick={acao.aoEscolher}
          >
            {acao.texto}
          </button>
        </>
      )}
    </div>
  );
}

/**
 * Alerta de atenção ou de erro: ícone, título e uma frase. O de erro é `role="alert"`
 * (anunciado na hora); o de atenção, `role="status"`. `acoes` entra no fim (ex.:
 * "Copiar a URL").
 */
export function Alerta({
  tom,
  titulo,
  children,
  acoes,
  role,
}: {
  tom: "atencao" | "erro";
  titulo: ReactNode;
  /** A frase que diz o que fazer. */
  children?: ReactNode;
  acoes?: ReactNode;
  /** Troca o papel padrão (ex.: "note" para um aviso fixo que não deve ser anunciado). */
  role?: AriaRole;
}) {
  return (
    <div className="alerta" data-tom={tom} role={role ?? (tom === "erro" ? "alert" : "status")}>
      <span className="alerta-icone">{tom === "erro" ? <IconeErro /> : <IconeAtencao />}</span>
      <div className="alerta-texto">
        <strong className="alerta-titulo">{titulo}</strong>
        {children && <> {children}</>}
      </div>
      {acoes && <div className="alerta-acoes">{acoes}</div>}
    </div>
  );
}
