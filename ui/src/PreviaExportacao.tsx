import { useEffect, useState } from "react";
import { mensagemDe } from "./analise";
import type { ApiConfiguracoes, PreviaExportacao as Previa } from "./configuracoes";

/** Espera depois da última tecla antes de remontar a prévia. */
export const ATRASO_PREVIA_MS = 400;

/**
 * Prévia da exportação em HTML (#166) no editor de textos: um documento de exemplo
 * exportado com o template e os tokens da base, com o texto em edição no lugar do
 * arquivo. O resultado vai num `iframe` com `sandbox` vazio: sem script, sem
 * navegação, isolado da tela do app.
 */
export function PreviaExportacao({
  api,
  arquivo,
  texto,
}: {
  api: ApiConfiguracoes;
  arquivo: string;
  texto: string;
}) {
  const [previa, setPrevia] = useState<Previa | null>(null);
  const [falha, setFalha] = useState<string | null>(null);

  useEffect(() => {
    let ativo = true;
    const t = setTimeout(() => {
      api
        .previaExportacao(arquivo, texto)
        .then((p) => {
          if (!ativo) return;
          setPrevia(p);
          setFalha(null);
        })
        .catch((e: unknown) => ativo && setFalha(mensagemDe(e)));
    }, ATRASO_PREVIA_MS);
    return () => {
      ativo = false;
      clearTimeout(t);
    };
  }, [api, arquivo, texto]);

  return (
    <div className="previa-exportacao">
      <p className="muted">
        Documento de exemplo exportado com este texto e os outros arquivos da exportação na
        base.
      </p>
      {falha && (
        <p className="error-text" role="alert">
          Não foi possível montar a prévia: {falha}
        </p>
      )}
      {previa && previa.avisos.length > 0 && (
        <ul className="lista-avisos attention-text" aria-label="Avisos da prévia">
          {previa.avisos.map((a) => (
            <li key={a}>{a}</li>
          ))}
        </ul>
      )}
      {previa ? (
        <iframe
          className="previa-exportacao-quadro"
          title="Prévia do documento exportado em HTML"
          sandbox=""
          srcDoc={previa.html}
        />
      ) : (
        !falha && <p className="muted">Montando a prévia…</p>
      )}
    </div>
  );
}
