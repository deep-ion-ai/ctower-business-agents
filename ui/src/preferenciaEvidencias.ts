import { useCallback, useState } from "react";

const CHAVE = "cba.evidenciasVisiveis";

/** As evidências ficam ocultas por padrão; só um "mostrar" explícito fica guardado. */
function lerGuardada(): boolean {
  try {
    return localStorage.getItem(CHAVE) === "1";
  } catch {
    return false;
  }
}

function guardar(visiveis: boolean) {
  try {
    localStorage.setItem(CHAVE, visiveis ? "1" : "0");
  } catch {
    // Sem armazenamento: a escolha vale só nesta tela.
  }
}

/**
 * Se as evidências de código de todas as afirmações estão à vista (#404), lembrado
 * entre as aberturas da análise e do app, como o terminal (`cba.terminalVisivel`).
 */
export function useEvidenciasVisiveis() {
  const [visiveis, setVisiveis] = useState(lerGuardada);
  const alternar = useCallback(() => {
    setVisiveis((v) => {
      guardar(!v);
      return !v;
    });
  }, []);
  return [visiveis, alternar] as const;
}
