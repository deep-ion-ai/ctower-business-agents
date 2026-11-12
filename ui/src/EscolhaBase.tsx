import { useEffect, useState, type FormEvent } from "react";
import { mensagemDe } from "./analise";
import type { ApiPrimeiroUso, AvaliacaoBase, EstadoPasta } from "./primeiroUso";

type Api = Pick<ApiPrimeiroUso, "avaliarBase" | "configurarBase" | "escolherPasta">;

const ESTADO: Record<EstadoPasta, string> = {
  nova: "A pasta ainda não existe; o app vai criá-la.",
  vazia: "A pasta está vazia.",
  base: "A pasta já é uma base de conhecimento; o app vai usá-la como está.",
  comArquivos: "A pasta já tem outros arquivos; a base vai ficar junto deles.",
};

/**
 * Passo da pasta da base de conhecimento (e item das configurações). Confere a pasta
 * a cada mudança (sem criar nada) e mostra a recusa: pasta temporária, dados do app ou
 * dentro do repositório de um sistema.
 */
export function EscolhaBase({
  api,
  atual,
  sugerida,
  aoConcluir,
  rotuloContinuar = "Usar esta pasta",
}: {
  api: Api;
  /** Base já configurada, se houver. */
  atual?: string | null;
  sugerida?: string | null;
  aoConcluir: (pasta: string) => void;
  rotuloContinuar?: string;
}) {
  const [pasta, setPasta] = useState(atual ?? sugerida ?? "");
  const [avaliacao, setAvaliacao] = useState<AvaliacaoBase | null>(null);
  const [falha, setFalha] = useState<string | null>(null);
  const [gravando, setGravando] = useState(false);

  useEffect(() => {
    const texto = pasta.trim();
    setAvaliacao(null);
    setFalha(null);
    if (!texto) return;
    let ativo = true;
    // Espera o analista parar de digitar.
    const t = setTimeout(() => {
      api
        .avaliarBase(texto)
        .then((a) => ativo && setAvaliacao(a))
        .catch((e: unknown) => ativo && setFalha(mensagemDe(e)));
    }, 300);
    return () => {
      ativo = false;
      clearTimeout(t);
    };
  }, [api, pasta]);

  const salvar = (ev: FormEvent) => {
    ev.preventDefault();
    setGravando(true);
    setFalha(null);
    api
      .configurarBase(pasta.trim())
      .then((gravada) => aoConcluir(gravada))
      .catch((e: unknown) => setFalha(mensagemDe(e)))
      .finally(() => setGravando(false));
  };

  const recusa = avaliacao?.recusa ?? null;

  return (
    <form className="passo-assistente" onSubmit={salvar} aria-label="Pasta da base de conhecimento">
      <h2>Onde guardar os documentos?</h2>
      <p>
        A base de conhecimento é uma pasta comum com os documentos das análises, em Markdown. Pode
        ser uma pasta sincronizada (OneDrive, Google Drive) ou um repositório git seu, mas não a
        pasta do código do sistema.
      </p>
      <label>
        Pasta da base (caminho completo){" "}
        <input
          className="caminho"
          value={pasta}
          onChange={(e) => setPasta(e.target.value)}
          spellCheck={false}
        />
      </label>{" "}
      <button
        className="btn"
        type="button"
        onClick={() => {
          api
            .escolherPasta("Pasta da base de conhecimento", pasta.trim() || undefined)
            .then((p) => p && setPasta(p))
            .catch((e: unknown) => setFalha(mensagemDe(e)));
        }}
      >
        Escolher pasta…
      </button>
      {sugerida && pasta.trim() !== sugerida && (
        <p>
          <button type="button" className="link" onClick={() => setPasta(sugerida)}>
            Usar a sugerida: {sugerida}
          </button>
        </p>
      )}
      {avaliacao && !recusa && avaliacao.estado && (
        <p className="muted" role="status">
          {ESTADO[avaliacao.estado]}
          {avaliacao.git && " Ela está num repositório git."}
        </p>
      )}
      {(recusa ?? falha) && (
        <p className="error-text" role="alert">
          {recusa ?? falha}
        </p>
      )}
      <div className="acoes">
        <button
          type="submit"
          className="btn btn-primary"
          disabled={!pasta.trim() || gravando || recusa !== null}
        >
          {rotuloContinuar}
        </button>
      </div>
    </form>
  );
}
