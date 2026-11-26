import { useState, type ReactNode } from "react";
import { rotuloDaEvidencia, type Afirmacao, type Confianca, type Evidencia } from "./analise";
import { AcoesAfirmacao, type ComandoAfirmacao, type Revisao } from "./AcoesRevisao";
import { BotaoCodigo } from "./BotaoCodigo";
import { Cartao, Selo, type TomSelo } from "./componentes";
import { premissaDaAfirmacao, ROTULO_VEREDITO, type Veredito } from "./premissaAfirmacao";
import { TextoAfirmacao } from "./TextoAfirmacao";
import "./configuracoes.css";

export const ROTULO_CONFIANCA: Record<Confianca, string> = {
  confirmado: "com evidência",
  inferido: "inferido",
  a_confirmar: "a confirmar",
};

/** Tom do selo de confiança: com evidência (concluído), inferido (andamento), a confirmar (atenção). */
export const TOM_CONFIANCA: Record<Confianca, TomSelo> = {
  confirmado: "concluido",
  inferido: "andamento",
  a_confirmar: "atencao",
};

/** Resultado da decisão do analista, no selo do cartão. */
const RESULTADO: Partial<Record<Afirmacao["estado"], { texto: string; tom: TomSelo }>> = {
  confirmada: { texto: "confirmada por você", tom: "concluido" },
  incorreta: { texto: "marcada como incorreta", tom: "erro" },
  aprofundar: { texto: "aprofundamento pedido", tom: "andamento" },
  "a-revalidar": { texto: "a revalidar: o código mudou", tom: "atencao" },
};

export function chaveDaEvidencia(e: Evidencia): string {
  return `${e.arquivo}:${e.linhas}`;
}

/** Selo do veredito (#404): o tom e o ícone vêm do significado; o texto vai junto. */
const SELO_VEREDITO: Record<Veredito, { tom: TomSelo; icone: string }> = {
  "nao-confere": { tom: "erro", icone: "✕" },
  "confere-em-parte": { tom: "atencao", icone: "–" },
  confere: { tom: "concluido", icone: "✓" },
};

/**
 * O texto da afirmação. No formato "Premissa do solicitante: …. Verificação: …"
 * (#404), a premissa e a verificação vêm em blocos com rótulo, o veredito num selo e
 * uma linha tracejada entre os dois; fora do formato, o texto como sempre.
 */
function CorpoAfirmacao({ texto }: { texto: string }) {
  const partes = premissaDaAfirmacao(texto);
  if (!partes) return <TextoAfirmacao texto={texto} />;
  const { premissa, verificacao } = partes;
  const selo = verificacao?.veredito ? SELO_VEREDITO[verificacao.veredito] : null;
  return (
    <div className="afirmacao-blocos">
      <div className="afirmacao-bloco" data-testid="premissa">
        <span className="afirmacao-rotulo">Premissa do solicitante</span>
        <TextoAfirmacao texto={premissa} />
      </div>
      {verificacao && (
        <div className="afirmacao-bloco afirmacao-verificacao" data-testid="verificacao">
          <div className="afirmacao-verificacao-cab">
            <span className="afirmacao-rotulo">Verificação</span>
            {selo && verificacao.veredito && (
              <Selo tom={selo.tom}>
                <span aria-hidden="true" className="selo-icone">
                  {selo.icone}
                </span>{" "}
                {ROTULO_VEREDITO[verificacao.veredito]}
              </Selo>
            )}
          </div>
          {verificacao.texto && <TextoAfirmacao texto={verificacao.texto} />}
        </div>
      )}
    </div>
  );
}

/**
 * Seletor "Mostrar evidências de código" do topo das afirmações (#404): mostra ou
 * esconde os chips de todos os cartões.
 */
export function SeletorEvidencias({
  ligado,
  aoAlternar,
}: {
  ligado: boolean;
  aoAlternar: () => void;
}) {
  return (
    <label className="seletor-evidencias">
      <button
        type="button"
        role="switch"
        className="interruptor"
        aria-checked={ligado}
        onClick={aoAlternar}
      >
        <span aria-hidden="true" />
      </button>
      Mostrar evidências de código
    </label>
  );
}

interface Props {
  afirmacao: Afirmacao;
  /** Ações de revisão; sem elas, só leitura. */
  revisao?: Revisao;
  /** É a afirmação em foco dos atalhos de teclado. */
  atual?: boolean;
  comando?: ComandoAfirmacao | null;
  /** Evidência aberta na aba Código (destacada entre os chips). */
  evidenciaAberta?: string | null;
  aoAbrirEvidencia: (e: Evidencia) => void;
  aoFocar?: () => void;
  /** Trecho aberto da evidência, quando mostrado dentro do cartão. */
  trecho?: ReactNode;
  /**
   * As evidências estão à vista (#404). Sem `aoAlternarEvidencias`, o cartão guarda
   * isso sozinho, ocultas por padrão.
   */
  evidenciasAbertas?: boolean;
  aoAlternarEvidencias?: () => void;
}

/**
 * Cartão de uma afirmação (#184): número, selo de confiança, texto, evidências como
 * chips, ações com rótulo e, depois da decisão, o selo do resultado com o comentário.
 */
export function CartaoAfirmacao({
  afirmacao: a,
  revisao,
  atual,
  comando,
  evidenciaAberta,
  aoAbrirEvidencia,
  aoFocar,
  trecho,
  evidenciasAbertas,
  aoAlternarEvidencias,
}: Props) {
  const [abertasAqui, setAbertasAqui] = useState(false);
  const controlado = !!aoAlternarEvidencias;
  const abertas = controlado ? !!evidenciasAbertas : abertasAqui;
  const alternarEvidencias = controlado ? aoAlternarEvidencias : () => setAbertasAqui((v) => !v);
  const contestada = a.estado === "a-revalidar" && !!a.contestacao;
  const resultado: { texto: string; tom: TomSelo } | undefined = contestada
    ? { texto: "a revalidar: contestada pela verificação independente", tom: "atencao" }
    : RESULTADO[a.estado];
  const semDecisao = !!revisao && a.estado === "registrada";
  const comentario =
    a.estado === "incorreta" ? a.comentario : a.estado === "aprofundar" ? a.pedido : undefined;
  return (
    <Cartao
      como="li"
      className="afirmacao"
      data-estado={a.estado}
      data-pendente={semDecisao || undefined}
      data-atual={atual || undefined}
      aria-current={atual ? "true" : undefined}
      data-testid={`afirmacao-${a.numero}`}
      tabIndex={-1}
      onFocus={aoFocar}
    >
      <div className="revisao-cabeca">
        <span className="revisao-numero" title={`Afirmação ${a.numero}`}>
          {a.numero}
        </span>
        <Selo tom={TOM_CONFIANCA[a.confianca]}>{ROTULO_CONFIANCA[a.confianca]}</Selo>
        {semDecisao && <Selo tom="atencao">sem decisão</Selo>}
        {resultado && <Selo tom={resultado.tom}>{resultado.texto}</Selo>}
        {a.texto_original && (
          <span className="muted editada" title={`Texto do agente: ${a.texto_original}`}>
            (texto editado pelo analista)
          </span>
        )}
        {a.evidencias.length > 0 && (
          <BotaoCodigo
            quantidade={a.evidencias.length}
            aberto={abertas}
            aoAlternar={alternarEvidencias}
          />
        )}
      </div>
      <CorpoAfirmacao texto={a.texto} />
      {comentario && <p className="comentario muted">{comentario}</p>}
      {a.contestacao && (
        <p className="comentario contestacao" data-testid={`contestacao-${a.numero}`}>
          <span className="muted">Contestação da verificação independente:</span>{" "}
          {a.contestacao.motivo}
        </p>
      )}
      {a.evidencias.length > 0 && abertas && (
        <div className="evidencias-afirmacao">
          <span className="evidencias-titulo">
            {a.evidencias.length === 1 ? "Evidência no código" : "Evidências no código"}
          </span>
          <ul className="evidencias" aria-label="Evidências">
            {a.evidencias.map((e) => {
              const chave = chaveDaEvidencia(e);
              return (
                <li key={chave}>
                  <button
                    type="button"
                    className="btn btn-sm evidencia chip-evidencia"
                    aria-pressed={evidenciaAberta === chave}
                    title={
                      e.trecho
                        ? `${e.arquivo}:${e.linhas}\n${e.trecho}`
                        : `${e.arquivo}:${e.linhas}`
                    }
                    onClick={() => aoAbrirEvidencia(e)}
                  >
                    {rotuloDaEvidencia(e)}
                  </button>
                  {e.estado === "a-revalidar" && <span className="muted"> a revalidar</span>}
                </li>
              );
            })}
          </ul>
        </div>
      )}
      {revisao && <AcoesAfirmacao afirmacao={a} revisao={revisao} comando={comando} />}
      {trecho}
    </Cartao>
  );
}
