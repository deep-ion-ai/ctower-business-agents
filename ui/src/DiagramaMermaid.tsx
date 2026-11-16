import { memo, useEffect, useState, type MouseEvent } from "react";
import { desenharMermaid, elementoDoId, temaDaTela, type ElementoDoDiagrama } from "./mermaid";

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
}: {
  codigo: string;
  /** Nome acessível da figura. */
  titulo?: string;
  aoClicar?: (elemento: ElementoDoDiagrama) => void;
}) {
  const [estado, setEstado] = useState<Estado>({ tipo: "carregando" });
  const [verCodigo, setVerCodigo] = useState(false);
  const tema = temaDaTela();

  useEffect(() => {
    let vivo = true;
    setEstado({ tipo: "carregando" });
    desenharMermaid(codigo, tema).then(
      (svg) => vivo && setEstado({ tipo: "pronto", svg }),
      (e: unknown) => vivo && setEstado({ tipo: "erro", mensagem: mensagemDoErro(e) }),
    );
    return () => {
      vivo = false;
    };
  }, [codigo, tema]);

  if (estado.tipo === "erro") {
    return (
      <div className="aviso-diagrama" role="alert">
        <p>
          <strong>Diagrama não renderizado.</strong> O código Mermaid tem um erro e foi mantido
          como texto; o restante do documento não foi afetado.
          {estado.mensagem && <span className="muted"> ({estado.mensagem})</span>}
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
    const elemento = elementoDoId(alvo?.getAttribute("data-id") ?? alvo?.getAttribute("id"));
    if (elemento) aoClicar(elemento);
  };

  return (
    <figure className="diagrama" aria-label={titulo ?? "Diagrama"}>
      {estado.tipo === "carregando" ? (
        <pre className="diagrama-carregando" aria-busy="true">
          <code>{codigo}</code>
        </pre>
      ) : (
        <div
          className="diagrama-figura"
          data-clicavel={aoClicar ? "true" : undefined}
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
