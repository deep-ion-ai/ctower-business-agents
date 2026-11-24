import { useState } from "react";
import type { ApiConhecimento } from "./conhecimento";
import { MudancasCodigo } from "./MudancasCodigo";
import { MapaProjeto } from "./TelaProjeto";

/**
 * Código (#319, §5.2 da revisão de UX): o mapa estrutural por pasta e "O que mudou
 * no código" do projeto escolhido, que antes ficavam no fim da tela Projeto.
 */
export function TelaCodigo({ api, projeto }: { api: ApiConhecimento; projeto: string }) {
  const [mudancas, setMudancas] = useState(false);

  return (
    <section className="page" aria-label="Código">
      <h2>Código</h2>
      {!projeto && <p className="muted">Nenhum projeto cadastrado.</p>}
      {projeto && api.mudancas && !mudancas && (
        <p>
          <button className="btn" type="button" onClick={() => setMudancas(true)}>
            O que mudou no código
          </button>
        </p>
      )}
      {projeto && api.mudancas && mudancas && (
        <MudancasCodigo api={api.mudancas} projeto={projeto} aoFechar={() => setMudancas(false)} />
      )}
      {projeto && <MapaProjeto api={api} projeto={projeto} />}
    </section>
  );
}
