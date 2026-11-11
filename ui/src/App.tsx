import { useEffect, useState } from "react";
import { getVersion } from "@tauri-apps/api/app";

export const PRODUTO = "Ctower Business Agents";

/** Tela inicial, ainda vazia: nome do produto e versão do app. */
export function App() {
  const [versao, setVersao] = useState<string | null>(null);

  useEffect(() => {
    let ativo = true;
    getVersion()
      .then((v) => ativo && setVersao(v))
      // Fora do Tauri (ex.: `npm run dev` no navegador) não há versão.
      .catch(() => ativo && setVersao(null));
    return () => {
      ativo = false;
    };
  }, []);

  return (
    <main className="inicio">
      <h1>{PRODUTO}</h1>
      <p className="versao" aria-label="Versão">
        {versao ? `Versão ${versao}` : " "}
      </p>
    </main>
  );
}
