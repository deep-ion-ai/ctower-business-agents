import { useState } from "react";
import { mensagemDe } from "./analise";
import type { ApiConfiguracoes, TextoEditavel } from "./configuracoes";

/** “Editar” (editor embutido) e “Abrir no editor do computador” de um texto editável. */
export function AbrirTexto({
  api,
  texto,
  aoAbrir,
  aoEditar,
}: {
  api: Pick<ApiConfiguracoes, "abrirTexto">;
  texto: TextoEditavel;
  aoAbrir: () => void;
  aoEditar: (arquivo: string) => void;
}) {
  const [aberto, setAberto] = useState<string | null>(null);
  const [falha, setFalha] = useState<string | null>(null);
  return (
    <>
      <button
        className="btn btn-primary"
        type="button"
        aria-label={`Editar ${texto.rotulo}`}
        onClick={() => aoEditar(texto.arquivo)}
      >
        Editar
      </button>{" "}
      <button
        className="btn"
        type="button"
        aria-label={`Abrir ${texto.rotulo} no editor do computador`}
        onClick={() => {
          setFalha(null);
          api
            .abrirTexto(texto.arquivo)
            .then((c) => {
              setAberto(c);
              aoAbrir();
            })
            .catch((e: unknown) => setFalha(mensagemDe(e)));
        }}
      >
        {texto.estado === "padrao"
          ? "Criar a cópia e abrir no computador"
          : "Abrir no editor do computador"}
      </button>
      {aberto && <span className="muted"> Aberto: {aberto}</span>}
      {falha && (
        <span className="error-text" role="alert">
          {" "}
          {falha}
        </span>
      )}
    </>
  );
}
