import { useCallback, useState } from "react";

const CHAVE = "cba.terminalVisivel";

/** O terminal aparece por padrão; só um "esconder" explícito fica guardado. */
function lerGuardada(): boolean {
  try {
    return localStorage.getItem(CHAVE) !== "0";
  } catch {
    return true;
  }
}

function guardar(visivel: boolean) {
  try {
    localStorage.setItem(CHAVE, visivel ? "1" : "0");
  } catch {
    // Sem armazenamento: a escolha vale só nesta tela.
  }
}

/**
 * Se o terminal da conversa está à vista, lembrado entre as aberturas do app (a
 * preferência é do usuário do sistema, como a largura da coluna). Esconder não
 * encerra a sessão: o terminal continua montado (#185).
 */
export function useTerminalVisivel() {
  const [visivel, setVisivel] = useState(lerGuardada);
  const alternar = useCallback(() => {
    setVisivel((v) => {
      guardar(!v);
      return !v;
    });
  }, []);
  return [visivel, alternar] as const;
}
