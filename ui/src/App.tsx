import { useEffect, useState } from "react";
import { getVersion } from "@tauri-apps/api/app";
import { ClientesIA } from "./ClientesIA";
import { TerminalTeste } from "./TerminalTeste";

export const PRODUTO = "Ctower Business Agents";

/**
 * Tela inicial: nome do produto, versão do app, estado dos clientes de IA e a tela
 * mínima do terminal embutido.
 */
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
      <ClientesIA />
      <TerminalTeste />
    </main>
  );
}
