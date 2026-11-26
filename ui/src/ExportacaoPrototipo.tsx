import { useState } from "react";
import { mensagemDe } from "./analise";
import {
  apiExportacaoPrototipoTauri,
  type ApiExportacaoPrototipo,
  type ExportacaoPrototipo as Resultado,
} from "./apiExportacaoPrototipo";
import { contar } from "./formatacao";

const CHAVE_DESTINO = "cba.exportacao.prototipo.destino";

/** Última pasta de destino escolhida; sem armazenamento, nenhuma. */
function lerDestino(): string | undefined {
  try {
    return localStorage.getItem(CHAVE_DESTINO) ?? undefined;
  } catch {
    return undefined;
  }
}

function lembrarDestino(destino: string) {
  try {
    localStorage.setItem(CHAVE_DESTINO, destino);
  } catch {
    // Sem armazenamento (janela privada, dados bloqueados): a escolha só vale agora.
  }
}

type Exportada = Extract<Resultado, { situacao: "exportada" }>;

/**
 * Exportar o protótipo (#380): o analista escolhe a pasta de destino e o app
 * grava nela uma pasta com o índice das telas, a cópia de cada tela (com a
 * navegação entre elas) e os tokens aprovados, para abrir no navegador sem o
 * app e sem rede. Pasta que já existe só é substituída depois de confirmar.
 */
export function ExportacaoPrototipo({
  projeto,
  analise,
  api = apiExportacaoPrototipoTauri,
}: {
  projeto: string;
  analise: string;
  api?: ApiExportacaoPrototipo;
}) {
  const [ocupado, setOcupado] = useState(false);
  const [confirmar, setConfirmar] = useState<{ destino: string; pasta: string } | null>(null);
  const [feita, setFeita] = useState<Exportada | null>(null);
  const [erro, setErro] = useState<string | null>(null);

  const executar = async (passo: () => Promise<string | null>, substituir: boolean) => {
    setErro(null);
    setFeita(null);
    setConfirmar(null);
    setOcupado(true);
    try {
      const destino = await passo();
      if (!destino) return;
      const r = await api.exportar(projeto, analise, destino, substituir);
      if (r.situacao === "pasta_existe") setConfirmar({ destino, pasta: r.pasta });
      else setFeita(r);
    } catch (e) {
      setErro(mensagemDe(e));
    } finally {
      setOcupado(false);
    }
  };

  const exportar = () =>
    executar(async () => {
      const destino = await api.escolherPasta(lerDestino());
      if (destino) lembrarDestino(destino);
      return destino;
    }, false);

  return (
    <section className="exportacao exportacao-prototipo" aria-label="Exportar protótipo">
      <button className="btn" type="button" disabled={ocupado} onClick={() => void exportar()}>
        Exportar protótipo…
      </button>{" "}
      <span className="muted">
        Pasta HTML com o índice e as telas, para abrir no navegador sem o app e sem rede.
      </span>
      {confirmar && (
        <div className="attention-text" role="alert">
          <p>
            A pasta <code>{confirmar.pasta}</code> já existe. Gravar a exportação por cima? Os
            arquivos de mesmo nome são substituídos; o resto da pasta fica como está.
          </p>
          <div className="acoes">
            <button
              type="button"
              className="btn btn-primary"
              disabled={ocupado}
              onClick={() => void executar(async () => confirmar.destino, true)}
            >
              Substituir
            </button>
            <button type="button" className="btn" onClick={() => setConfirmar(null)}>
              Cancelar
            </button>
          </div>
        </div>
      )}
      {feita && (
        <p role="status">
          Protótipo exportado para <code>{feita.pasta}</code> (
          {contar(feita.telas, "tela", "telas")}
          {feita.semPrototipo > 0 &&
            `; ${contar(feita.semPrototipo, "tela", "telas")} sem protótipo ainda`}
          ). Abra o <code>index.html</code> no navegador.
        </p>
      )}
      {erro && (
        <p className="error-text" role="alert">
          {erro}
        </p>
      )}
    </section>
  );
}
