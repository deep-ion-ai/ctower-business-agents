import { useId, type ReactNode } from "react";
import "./componentes.css";

type Nivel = 2 | 3 | 4;

/**
 * Título de seção (16/600), com espaço à direita para as ações da seção. Nunca em
 * caixa-alta: a caixa-alta fica só para o rótulo de campo.
 */
export function TituloSecao({
  children,
  acoes,
  nivel = 2,
  id,
}: {
  children: ReactNode;
  /** Ações da seção, à direita do título (ex.: um botão ou um `MenuAcoes`). */
  acoes?: ReactNode;
  /** Nível do cabeçalho (`h2` por padrão). */
  nivel?: Nivel;
  id?: string;
}) {
  const Cabecalho = `h${nivel}` as const;
  return (
    <div className="titulo-secao">
      <Cabecalho id={id}>{children}</Cabecalho>
      {acoes && <div className="titulo-secao-acoes">{acoes}</div>}
    </div>
  );
}

/**
 * Seção da tela: bloco branco sobre o fundo cinza, com {@link TituloSecao}. É uma
 * região (`<section>`) nomeada pelo título; sem título, pelo `rotulo`.
 */
export function Secao({
  titulo,
  acoes,
  nivel,
  rotulo,
  className,
  children,
}: {
  titulo?: ReactNode;
  acoes?: ReactNode;
  nivel?: Nivel;
  /** Nome acessível quando não há título visível (ou quando o título não é texto). */
  rotulo?: string;
  className?: string;
  children?: ReactNode;
}) {
  const id = useId();
  const nomeado = titulo !== undefined && !rotulo;
  return (
    <section
      className={className ? `secao ${className}` : "secao"}
      aria-labelledby={nomeado ? id : undefined}
      aria-label={rotulo}
    >
      {titulo !== undefined && (
        <TituloSecao id={id} nivel={nivel} acoes={acoes}>
          {titulo}
        </TituloSecao>
      )}
      {children}
    </section>
  );
}

/**
 * Cartão: um item com decisão própria (uma pergunta, uma afirmação). Com `rotulo`,
 * vira um `<article>` nomeado.
 */
export function Cartao({
  rotulo,
  como = "article",
  className,
  children,
}: {
  rotulo?: string;
  /** Elemento do cartão: `article` (padrão), `div` ou `li` (dentro de uma lista). */
  como?: "article" | "div" | "li";
  className?: string;
  children: ReactNode;
}) {
  const Elemento = como;
  return (
    <Elemento className={className ? `cartao ${className}` : "cartao"} aria-label={rotulo}>
      {children}
    </Elemento>
  );
}

/** Lista de {@link ItemLista}, sem marcadores. */
export function ListaItens({ rotulo, children }: { rotulo?: string; children: ReactNode }) {
  return (
    <ul className="lista-itens" aria-label={rotulo}>
      {children}
    </ul>
  );
}

/**
 * Item de lista: uma linha com divisor (registro, linha do tempo), com metadado e
 * ações (ex.: `BotaoIcone`) à direita.
 */
export function ItemLista({
  children,
  meta,
  acoes,
}: {
  children: ReactNode;
  /** Data, origem, autor: texto de apoio à direita. */
  meta?: ReactNode;
  acoes?: ReactNode;
}) {
  return (
    <li className="item-lista">
      <div className="item-lista-corpo">{children}</div>
      {meta && <span className="item-lista-meta">{meta}</span>}
      {acoes && <div className="item-lista-acoes">{acoes}</div>}
    </li>
  );
}
