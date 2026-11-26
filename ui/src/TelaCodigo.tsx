import { useState } from "react";
import { BarraAcoes, EstadoVazio } from "./componentes";
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
      {!projeto && <EstadoVazio>Nenhum projeto cadastrado.</EstadoVazio>}
      {projeto && api.mudancas && !mudancas && (
        <BarraAcoes
          secundarias={[{ texto: "O que mudou no código", aoEscolher: () => setMudancas(true) }]}
          rotulo="Ações do código"
        />
      )}
      {projeto && api.mudancas && mudancas && (
        <MudancasCodigo api={api.mudancas} projeto={projeto} aoFechar={() => setMudancas(false)} />
      )}
      {projeto && <MapaProjeto api={api} projeto={projeto} />}
    </section>
  );
}
