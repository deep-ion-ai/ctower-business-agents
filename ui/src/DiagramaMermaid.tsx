import { memo, useEffect, useState, type MouseEvent } from "react";
import {
  desenharMermaid,
  elementoDoId,
  temaDaTela,
  type ElementoDoDiagrama,
} from "./mermaid";

type Estado =
  | { tipo: "carregando" }
  | { tipo: "pronto"; svg: string }
  | { tipo: "erro"; mensagem: string };

function mensagemDoErro(e: unknown): string {
  const texto = e instanceof Error ? e.message : String(e);
  // A primeira linha basta ("Parse error on line 3:"); o resto repete o código.
  return texto.split("\n")[0].trim();
}

/**
 * Diagrama Mermaid desenhado na tela (#118), sem rede. Enquanto a biblioteca carrega,
 * mostra o código; código inválido não quebra nada: aparece um aviso com o erro e o
 * código como texto. "Ver Mermaid" mostra o código do diagrama desenhado.
 *
 * O SVG vem do Mermaid em `securityLevel: "strict"` (rótulos sanitizados, sem
 * `click` nem scripts) e é o único HTML que a tela insere sem passar pelo React.
 * Com `aoClicar`, o clique num nó ou numa aresta do diagrama gerado pelo app
 * (`n1`…, `L_n1_n2_0`) devolve o elemento, para a tela abrir a evidência dele.
 */
export const DiagramaMermaid = memo(function DiagramaMermaid({
  codigo,
  titulo,
  aoClicar,
  ampliavel,
}: {
  codigo: string;
  /** Nome acessível da figura. */
  titulo?: string;
  aoClicar?: (elemento: ElementoDoDiagrama) => void;
  /** Oferece "Tela cheia" (diagrama grande, texto pequeno no painel). */
  ampliavel?: boolean;
}) {
  const [estado, setEstado] = useState<Estado>({ tipo: "carregando" });
  const [verCodigo, setVerCodigo] = useState(false);
  const [cheia, setCheia] = useState(false);
  const [zoom, setZoom] = useState(1);
  const tema = temaDaTela();

  useEffect(() => {
    let vivo = true;
    setEstado({ tipo: "carregando" });
    desenharMermaid(codigo, tema).then(
      (svg) => vivo && setEstado({ tipo: "pronto", svg }),
      (e: unknown) =>
        vivo && setEstado({ tipo: "erro", mensagem: mensagemDoErro(e) }),
    );
    return () => {
      vivo = false;
    };
  }, [codigo, tema]);

  if (estado.tipo === "erro") {
    return (
      <div className="aviso-diagrama" role="alert">
        <p>
          <strong>Diagrama não renderizado.</strong> O código Mermaid tem um
          erro e foi mantido como texto; o restante do documento não foi
          afetado.
          {estado.mensagem && (
            <span className="muted"> ({estado.mensagem})</span>
          )}
        </p>
        <pre>
          <code>{codigo}</code>
        </pre>
      </div>
    );
  }

  const clicar = (e: MouseEvent<HTMLDivElement>) => {
    if (!aoClicar) return;
    const alvo = (e.target as Element).closest?.("[data-id], g.node");
    const elemento = elementoDoId(
      alvo?.getAttribute("data-id") ?? alvo?.getAttribute("id"),
    );
    if (elemento) aoClicar(elemento);
  };

  return (
    <figure
      className="diagrama"
      data-tela-cheia={cheia || undefined}
      aria-label={titulo ?? "Diagrama"}
    >
      {ampliavel && estado.tipo === "pronto" && (
        <div
          className="diagrama-zoom"
          role="group"
          aria-label="Zoom do diagrama"
        >
          <button
            type="button"
            className="btn btn-sm"
            aria-label="Diminuir"
            disabled={zoom <= 0.5}
            onClick={() =>
              setZoom((z) => Math.max(0.5, +(z - 0.25).toFixed(2)))
            }
          >
            −
          </button>
          <span className="muted" aria-live="polite">
            {Math.round(zoom * 100)}%
          </span>
          <button
            type="button"
            className="btn btn-sm"
            aria-label="Ampliar"
            disabled={zoom >= 4}
            onClick={() => setZoom((z) => Math.min(4, +(z + 0.25).toFixed(2)))}
          >
            +
          </button>
          <button
            type="button"
            className="btn btn-sm"
            aria-pressed={cheia}
            onClick={() => setCheia((c) => !c)}
          >
            {cheia ? "Sair da tela cheia" : "Tela cheia"}
          </button>
        </div>
      )}
      {estado.tipo === "carregando" ? (
        <pre className="diagrama-carregando" aria-busy="true">
          <code>{codigo}</code>
        </pre>
      ) : (
        <div
          className="diagrama-figura"
          data-clicavel={aoClicar ? "true" : undefined}
          style={zoom === 1 ? undefined : { ["--zoom" as string]: zoom }}
          data-zoom={zoom === 1 ? undefined : zoom}
          onClick={clicar}
          dangerouslySetInnerHTML={{ __html: estado.svg }}
        />
      )}
      <button
        type="button"
        className="btn btn-sm"
        aria-expanded={verCodigo}
        onClick={() => setVerCodigo((v) => !v)}
      >
        {verCodigo ? "Ocultar Mermaid" : "Ver Mermaid"}
      </button>
      {verCodigo && (
        <pre className="diagrama-codigo">
          <code>{codigo}</code>
        </pre>
      )}
    </figure>
  );
});
